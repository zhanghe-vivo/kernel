use crate::{
    new_spinlock, object, static_init::UnsafeStaticInit, sync::SpinLock, thread::RtThread,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{pin::Pin, ptr::NonNull};
use pinned_init::{pin_data, pin_init, PinInit};

#[cfg(feature = "RT_USING_SMP")]
use crate::{
    c_str,
    str::CStr,
    thread::{ThreadEntryFn, ThreadWithStack},
};

#[cfg(feature = "RT_USING_SMP")]
const ZOMBIE_THREAD_STACK_SIZE: usize = rt_bindings::IDLE_THREAD_STACK_SIZE as usize;
#[cfg(feature = "RT_USING_SMP")]
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

#[cfg(not(feature = "RT_USING_SMP"))]
#[pin_data]
pub(crate) struct ZombieManager {
    #[pin]
    zombies_list: SpinLock<ListHead>,
}

#[cfg(feature = "RT_USING_SMP")]
#[pin_data]
pub(crate) struct ZombieManager {
    #[pin]
    zombies_list: SpinLock<ListHead>,
    #[pin]
    thread: ThreadWithStack<ZOMBIE_THREAD_STACK_SIZE>,
    #[pin]
    sem: RtSemaphore,
}

impl ZombieManager {
    #[cfg(not(feature = "RT_USING_SMP"))]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            zombies_list <- new_spinlock!(ListHead::new()),
        })
    }

    #[cfg(feature = "RT_USING_SMP")]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            zombies_list <- new_spinlock!(ListHead::new()),
            thread <- ThreadWithStack::new(ZOMBIE_NAME, Self::zombie_thread_entry as ThreadEntryFn,
                core::ptr::null_mut(), (rt_bindings::RT_THREAD_PRIORITY_MAX - 2) as u8, 32),
            sem <- unsafe {
                pin_init_from_closure::<_, ::core::convert::Infallible>(|slot| {
                    (*slot).init(ZOMBIE_NAME.as_char_ptr(), 0, rt_bindings::RT_IPC_FLAG_FIFO as u8);
                    Ok(())
                })
            },
        })
    }

    #[cfg(feature = "RT_USING_SMP")]
    extern "C" fn zombie_thread_entry(parameter: *mut core::ffi::c_void) {
        let _ = parameter;

        loop {
            unsafe {
                let ret = (&mut ZOMBIE_MANAGER.sem as *mut _).take(rt_bindings::RT_WAITING_FOREVER);
                assert!(ret == rt_bindings::RT_EOK as i32);
                ZOMBIE_MANAGER.reclaim();
            }
        }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn start_up(&mut self) {
        self.thread.start();
    }

    pub(crate) fn reclaim(&self) {
        loop {
            // get defunct thread
            if let Some(thread) = self.zombie_dequeue() {
                let th = thread.as_ptr();
                #[cfg(feature = "RT_USING_SIGNALS")]
                unsafe {
                    rt_bindings::rt_thread_free_sig(th);
                }
                // if it's a system object, detach it
                let object_is_systemobject =
                    object::rt_object_is_systemobject(th as rt_bindings::rt_object_t);
                if object_is_systemobject == rt_bindings::RT_TRUE as i32 {
                    // detach this object
                    object::rt_object_detach(th as rt_bindings::rt_object_t);
                }

                // invoke thread cleanup
                let func = unsafe { (*th).get_cleanup_fn() };
                func(th);

                // if need free, delete it
                #[cfg(feature = "RT_USING_HEAP")]
                if object_is_systemobject == rt_bindings::RT_FALSE as i32 {
                    // delete thread object
                    object::rt_object_delete(th as rt_bindings::rt_object_t);
                }
            } else {
                break;
            }
        }
    }

    pub(crate) fn zombie_enqueue(&mut self, thread: &mut RtThread) {
        let list = self.zombies_list.lock();
        unsafe { Pin::new_unchecked(&mut thread.tlist).insert_next(&*list) };
        drop(list);
        #[cfg(feature = "RT_USING_SMP")]
        unsafe {
            ((&mut self.sem) as *mut _).release();
        }
    }

    pub(crate) fn zombie_dequeue(&self) -> Option<NonNull<RtThread>> {
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
