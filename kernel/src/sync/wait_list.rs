use core::{
    convert::TryFrom,
    pin::Pin,
    ptr::{self, NonNull},
};

use bluekernel_infra::list::doubly_linked_list::{LinkedListNode, ListHead};
use pinned_init::{pin_data, pin_init, PinInit};

use crate::{
    doubly_linked_list_for_each,
    error::{code, Error},
    thread::{SuspendFlag, Thread},
};

/// WaitList working mode, Fifo or Priority
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum WaitMode {
    Fifo = 0,
    Priority = 1,
}

impl TryFrom<u32> for WaitMode {
    type Error = &'static str;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(WaitMode::Fifo),
            1 => Ok(WaitMode::Priority),
            _ => Err("Invalid value for WaitMode"),
        }
    }
}

/// WaitList for pending threads
#[derive(Debug)]
#[repr(C)]
#[pin_data]
pub(crate) struct WaitList {
    /// WaitList impl by ListHead
    #[pin]
    pub(crate) wait_list: ListHead,
    /// WaitList working mode, FIFO or PRIO
    mode: WaitMode,
}

impl WaitList {
    #[inline]
    pub(crate) fn new(mode: WaitMode) -> impl PinInit<Self> {
        pin_init!(Self {
            wait_list <- ListHead::new(),
            mode
        })
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn init(&mut self, mode: WaitMode) {
        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.wait_list as *mut ListHead);
        }
        self.mode = mode;
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn mode(&self) -> WaitMode {
        self.mode
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.wait_list.is_empty()
    }

    #[inline]
    pub(crate) fn head(&self) -> Option<NonNull<LinkedListNode>> {
        self.wait_list.next()
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn tail(&self) -> Option<NonNull<LinkedListNode>> {
        self.wait_list.prev()
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn count(&self) -> usize {
        self.wait_list.size()
    }

    pub(crate) fn wait(
        &mut self,
        thread: &mut Thread,
        pending_mode: SuspendFlag,
    ) -> Result<(), Error> {
        if !thread.stat.is_suspended() {
            if !thread.suspend(pending_mode) {
                return Err(code::ERROR);
            }
        }

        match self.mode {
            WaitMode::Fifo => {
                unsafe {
                    Pin::new_unchecked(&mut self.wait_list)
                        .push_back(Pin::new_unchecked(&mut thread.list_node))
                };
            }
            WaitMode::Priority => {
                doubly_linked_list_for_each!(node, &self.wait_list, {
                    let queued_thread_ptr =
                        unsafe { crate::thread_list_node_entry!(node.as_ptr()) as *mut Thread };

                    let queued_thread = unsafe { &mut *queued_thread_ptr };

                    if thread.priority.get_current() < queued_thread.priority.get_current() {
                        unsafe {
                            Pin::new_unchecked(&mut thread.list_node)
                                .insert_after(Pin::new_unchecked(&mut queued_thread.list_node));
                        }
                        break;
                    }
                });

                if ptr::eq(node.as_ptr(), &self.wait_list) {
                    unsafe {
                        Pin::new_unchecked(&mut self.wait_list)
                            .push_back(Pin::new_unchecked(&mut thread.list_node))
                    };
                }
            }
        }

        Ok(())
    }

    // return need schedule
    pub(crate) fn wake(&mut self) -> bool {
        if let Some(node) = unsafe { Pin::new_unchecked(&mut self.wait_list).pop_front() } {
            let thread = unsafe { &mut *crate::thread_list_node_entry!(node.as_ptr()) };
            thread.error = code::EOK;
            return thread.resume();
        }

        false
    }

    pub(crate) fn wake_all(&mut self) -> bool {
        let mut ret = false;
        while let Some(node) = unsafe { Pin::new_unchecked(&mut self.wait_list).pop_front() } {
            let thread = unsafe { &mut *crate::thread_list_node_entry!(node.as_ptr()) };
            thread.error = code::EOK;
            let _ = thread.resume();
            ret = true;
        }

        ret
    }

    /// Iterate over each node in the wait list.
    ///
    /// # Safety
    ///
    /// Do not modify the wait list while iterating over it.
    #[allow(dead_code)]
    pub(crate) unsafe fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(&LinkedListNode),
    {
        crate::doubly_linked_list_for_each!(node, &self.wait_list, {
            f(node);
        });
    }
}
