use crate::{
    new_spinlock, object::KernelObject, static_init::UnsafeStaticInit, sync::SpinLock,
    thread::Thread,
};
use alloc::alloc::{dealloc, Layout};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{pin::Pin, ptr::NonNull};
use pinned_init::{pin_data, pin_init, PinInit};

#[cfg(feature = "smp")]
use crate::{
    blue_kconfig, c_str, clock, scheduler,
    str::CStr,sync::ipc_common,error::code,
    thread::{ThreadEntryFn, ThreadWithStack},
};

#[cfg(feature = "smp")]
const ZOMBIE_THREAD_STACK_SIZE: usize = blue_kconfig::IDLE_THREAD_STACK_SIZE as usize;
#[cfg(feature = "smp")]
const ZOMBIE_NAME: &'static crate::ctr::CStr = crate::c_str!("zombie");

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
                core::ptr::null_mut(), (blue_kconfig::THREAD_PRIORITY_MAX - 2) as u8, 32),
            sem <- unsafe {
                pin_init_from_closure::<_, ::core::convert::Infallible>(|slot| {
                    (*slot).init(ZOMBIE_NAME.as_char_ptr(), 0, ipc_common::IPC_WAIT_MODE_FIFO as u8);
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
        loop {
            // get defunct thread
            if let Some(thread) = self.zombie_dequeue() {
                let th = thread.as_ptr();
                #[cfg(feature = "signals")]
                unsafe {
                    rt_bindings::rt_thread_free_sig(th);
                }
                // if it's a system object, detach it
                let object_is_systemobject = unsafe { (*th).is_static_kobject() };
                if object_is_systemobject {
                    // detach this object
                    unsafe { (*th).parent.detach() };
                }

                // invoke thread cleanup
                let func = unsafe { (*th).get_cleanup_fn() };
                func(th);

                // if need free, delete it
                #[cfg(feature = "heap")]
                if !object_is_systemobject {
                    // SAFETY: thread is a dynamic object, delete thread stack and thread object manually.
                    unsafe {
                        // free stack
                        let layout = Layout::from_size_align_unchecked(
                            (*th).stack().size(),
                            blue_kconfig::ALIGN_SIZE as usize,
                        );
                        dealloc((*th).stack().bottom_ptr() as *mut u8, layout);
                        // delete thread object
                        (*th).parent.delete()
                    };
                }
            } else {
                break;
            }
        }
    }

    pub(crate) fn zombie_enqueue(&mut self, thread: &mut Thread) {
        let list = self.zombies_list.lock();
        unsafe { Pin::new_unchecked(&mut thread.tlist).insert_next(&*list) };
        drop(list);
        #[cfg(feature = "smp")]
        unsafe {
            ((&mut self.sem) as *mut _).release();
        }
    }

    pub(crate) fn zombie_dequeue(&self) -> Option<NonNull<Thread>> {
        let list = self.zombies_list.lock();
        if let Some(mut thread_list) = (*list).next() {
            unsafe {
                let list = thread_list.as_ptr();
                let th = NonNull::new_unchecked(crate::thread_list_node_entry!(list));
                let pin_list = Pin::new_unchecked(thread_list.as_mut());
                // pin!((*thread_list.as_ptr()));
                pin_list.remove();
                return Some(th);
            }
        }
        None
    }
}
