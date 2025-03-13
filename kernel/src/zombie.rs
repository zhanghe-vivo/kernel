use crate::{
    new_spinlock, object::KernelObject, static_init::UnsafeStaticInit, sync::SpinLock,
    thread::Thread,
};
use alloc::alloc::{dealloc, Layout};
use bluekernel_infra::list::doubly_linked_list::ListHead;
use core::{pin::Pin, ptr::NonNull};
use pinned_init::{pin_data, pin_init, PinInit};

#[cfg(feature = "smp")]
use crate::{
    bluekernel_kconfig, c_str, clock,
    error::code,
    scheduler,
    sync::ipc_common,
    thread::{ThreadEntryFn, ThreadWithStack},
};

#[cfg(feature = "smp")]
const ZOMBIE_THREAD_STACK_SIZE: usize = bluekernel_kconfig::IDLE_THREAD_STACK_SIZE as usize;
#[cfg(feature = "smp")]
const ZOMBIE_NAME: &'static core::ffi::CStr = crate::c_str!("zombie");

pub(crate) static mut ZOMBIE_MANAGER: UnsafeStaticInit<ZombieManager, ZombieManagerInit> =
    UnsafeStaticInit::new(ZombieManagerInit);

pub(crate) struct ZombieManagerInit;
unsafe impl PinInit<ZombieManager> for ZombieManagerInit {
    unsafe fn __pinned_init(
        self,
        slot: *mut ZombieManager,
    ) -> Result<(), core::convert::Infallible> {
        let init = ZombieManager::new();
        unsafe { init.__pinned_init(slot) }
    }
}

#[cfg(not(feature = "smp"))]
#[pin_data]
pub(crate) struct ZombieManager {
    #[pin]
    zombies_list: SpinLock<ListHead>,
}

#[cfg(feature = "smp")]
#[pin_data]
pub(crate) struct ZombieManager {
    #[pin]
    zombies_list: SpinLock<ListHead>,
    #[pin]
    thread: ThreadWithStack<ZOMBIE_THREAD_STACK_SIZE>,
    #[pin]
    sem: Semaphore,
}

impl ZombieManager {
    #[cfg(not(feature = "smp"))]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            zombies_list <- new_spinlock!(ListHead::new()),
        })
    }

    #[cfg(feature = "smp")]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            zombies_list <- new_spinlock!(ListHead::new()),
            thread <- ThreadWithStack::new(ZOMBIE_NAME, Self::zombie_thread_entry as ThreadEntryFn,
                core::ptr::null_mut(), (bluekernel_kconfig::THREAD_PRIORITY_MAX - 2) as u8, 32),
            sem <- unsafe {
                pin_init_from_closure::<_, ::core::convert::Infallible>(|slot| {
                    (*slot).init(ZOMBIE_NAME.as_ptr(), 0, ipc_common::WaitMode::Fifo);
                    Ok(())
                })
            },
        })
    }

    #[cfg(feature = "smp")]
    extern "C" fn zombie_thread_entry(parameter: *mut core::ffi::c_void) {
        let _ = parameter;

        loop {
            unsafe {
                let ret = (&mut ZOMBIE_MANAGER.sem as *mut _).take(clock::WAITING_FOREVER);
                assert!(ret == code::EOK.to_errno());
                ZOMBIE_MANAGER.reclaim();
            }
        }
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn start_up(&mut self) {
        self.thread.start();
    }

    pub(crate) fn reclaim(&self) {
        while let Some(thread) = self.zombie_dequeue() {
            let th = thread.as_ptr();
            #[cfg(feature = "signals")]
            unsafe {
                rt_bindings::rt_thread_free_sig(th);
            }

            let func = unsafe { (*th).get_cleanup_fn() };
            func(th);

            if unsafe { (*th).should_free_stack() } {
                unsafe {
                    let layout = Layout::from_size_align_unchecked(
                        (*th).stack().size(),
                        bluekernel_kconfig::ALIGN_SIZE as usize,
                    );
                    dealloc((*th).stack().bottom_ptr() as *mut u8, layout);
                }
            }

            // If it's a system object, just detach it. Must be noted, object.parent.delete() implies
            // object.parent.detach().
            let object_is_systemobject = unsafe { (*th).is_static_kobject() };
            if object_is_systemobject {
                unsafe { (*th).parent.detach() };
                continue;
            }

            unsafe { (*th).parent.delete() };
        }
    }

    pub(crate) fn zombie_enqueue(&mut self, thread: &mut Thread) {
        let mut list = self.zombies_list.lock();
        unsafe {
            Pin::new_unchecked(&mut *list).push_back(Pin::new_unchecked(&mut thread.list_node))
        };
        drop(list);
        #[cfg(feature = "smp")]
        unsafe {
            ((&mut self.sem) as *mut _).release();
        }
    }

    pub(crate) fn zombie_dequeue(&self) -> Option<NonNull<Thread>> {
        let mut list = self.zombies_list.lock();
        if let Some(thread_list) = unsafe { Pin::new_unchecked(&mut *list).pop_front() } {
            let th = unsafe {
                NonNull::new_unchecked(crate::thread_list_node_entry!(thread_list.as_ptr()))
            };
            return Some(th);
        }
        None
    }
}
