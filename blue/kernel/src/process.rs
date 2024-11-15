use crate::{
    klibc::rt_strncmp,
    linked_list::ListHead,
    object::*,
    scheduler::{rt_enter_critical, rt_exit_critical},
    static_init::UnsafeStaticInit,
    sync::RawSpin,
};

use ::rt_bindings;
use core::{
    ffi,
    pin::Pin,
    ptr::{self, addr_of_mut},
};
use pinned_init::*;

pub(crate) static mut KPROCESS: UnsafeStaticInit<Kprocess, KprocessInit> =
    UnsafeStaticInit::new(KprocessInit);

pub(crate) struct KprocessInit;
unsafe impl PinInit<Kprocess> for KprocessInit {
    unsafe fn __pinned_init(self, slot: *mut Kprocess) -> Result<(), core::convert::Infallible> {
        let init = Kprocess::new();
        unsafe { init.__pinned_init(slot) }
    }
}

/// The kernel process
#[pin_data]
pub(crate) struct Kprocess {
    base: KObjectBase,
    ///not use yet
    #[pin]
    sibling: ListHead,
    ///not use yet
    #[pin]
    children: ListHead,
    #[pin]
    threas: ListHead,
    #[cfg(feature = "RT_USING_SEMAPHORE")]
    #[pin]
    semaphore: ListHead,
    #[cfg(feature = "RT_USING_MUTEX")]
    #[pin]
    mutex: ListHead,
    #[cfg(feature = "RT_USING_EVENT")]
    #[pin]
    event: ListHead,
    #[cfg(feature = "RT_USING_MAILBOX")]
    #[pin]
    mailbox: ListHead,
    #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
    #[pin]
    msgqueue: ListHead,
    #[cfg(feature = "RT_USING_MEMHEAP")]
    #[pin]
    memheap: ListHead,
    #[cfg(feature = "RT_USING_MEMPOOL")]
    #[pin]
    mempool: ListHead,
    #[cfg(feature = "RT_USING_DEVICE")]
    #[pin]
    device: ListHead,
    #[pin]
    timer: ListHead,
    #[cfg(feature = "RT_USING_HEAP")]
    #[pin]
    memory: ListHead,
    pid: u64,
    lock: RawSpin,
}

impl Kprocess {
    fn new() -> impl PinInit<Self> {
        let init = move |slot: *mut Self| unsafe {
            let cur_ref = &mut *slot;
            KObjectBase::init(
                &mut *(slot as *mut KObjectBase),
                ObjectClassType::ObjectClassProcess as u8,
                crate::c_str!("kprocess").as_ptr() as *const i8,
            );
            let _ = ListHead::new().__pinned_init(&mut cur_ref.sibling as *mut ListHead);
            let _ = ListHead::new().__pinned_init(&mut cur_ref.children as *mut ListHead);
            let _ = ListHead::new().__pinned_init(&mut cur_ref.threas as *mut ListHead);
            #[cfg(feature = "RT_USING_SEMAPHORE")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.semaphore as *mut ListHead);
            #[cfg(feature = "RT_USING_MUTEX")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.mutex as *mut ListHead);
            #[cfg(feature = "RT_USING_EVENT")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.event as *mut ListHead);
            #[cfg(feature = "RT_USING_MAILBOX")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.mailbox as *mut ListHead);
            #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.msgqueue as *mut ListHead);
            #[cfg(feature = "RT_USING_MEMHEAP")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.memheap as *mut ListHead);
            #[cfg(feature = "RT_USING_MEMPOOL")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.mempool as *mut ListHead);
            #[cfg(feature = "RT_USING_DEVICE")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.device as *mut ListHead);
            let _ = ListHead::new().__pinned_init(&mut cur_ref.timer as *mut ListHead);
            #[cfg(feature = "RT_USING_HEAP")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.memory as *mut ListHead);
            cur_ref.pid = 0;
            cur_ref.lock = RawSpin::new();
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    fn get_object_list(&self, object_tpye: u8) -> &ListHead {
        let process = Kprocess::get_process();
        let _ = process.lock.acquire();
        match object_tpye & (!OBJECT_CLASS_STATIC) {
            x if x == ObjectClassType::ObjectClassThread as u8 => &process.threas,
            #[cfg(feature = "RT_USING_SEMAPHORE")]
            x if x == ObjectClassType::ObjectClassSemaphore as u8 => &process.semaphore,
            #[cfg(feature = "RT_USING_MUTEX")]
            x if x == ObjectClassType::ObjectClassMutex as u8 => &process.mutex,
            #[cfg(feature = "RT_USING_EVENT")]
            x if x == ObjectClassType::ObjectClassEvent as u8 => &process.event,
            #[cfg(feature = "RT_USING_MAILBOX")]
            x if x == ObjectClassType::ObjectClassMailBox as u8 => &process.mailbox,
            #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
            x if x == ObjectClassType::ObjectClassMessageQueue as u8 => &process.msgqueue,
            #[cfg(feature = "RT_USING_MEMHEAP")]
            x if x == ObjectClassType::ObjectClassMemHeap as u8 => &process.memheap,
            #[cfg(feature = "RT_USING_MEMPOOL")]
            x if x == ObjectClassType::ObjectClassMemPool as u8 => &process.mempool,
            #[cfg(feature = "RT_USING_DEVICE")]
            x if x == ObjectClassType::ObjectClassDevice as u8 => &process.device,
            x if x == ObjectClassType::ObjectClassTimer as u8 => &process.timer,
            #[cfg(feature = "RT_USING_HEAP")]
            x if x == ObjectClassType::ObjectClassMemory as u8 => &process.memory,
            _ => unreachable!("not a kernel object type!"),
        }
    }

    fn get_process() -> &'static mut UnsafeStaticInit<Kprocess, KprocessInit> {
        let process;
        unsafe {
            process = &mut *addr_of_mut!(KPROCESS);
        }
        process
    }

    fn insert(&mut self, object_tpye: u8, node: &mut ListHead) {
        let list = self.get_object_list(object_tpye);
        let _ = self.lock.acquire();
        unsafe {
            Pin::new_unchecked(node).insert_next(list);
        }
    }

    #[cfg(feature = "RT_USING_DEBUG")]
    fn addr_detect(&mut self, object_tpye: u8, ptr: &mut KObjectBase) {
        let list = self.get_object_list(object_tpye);
        let _ = self.lock.acquire();
        crate::list_head_for_each!(node, list, {
            let obj = unsafe { crate::list_head_entry!(node.as_ptr(), KObjectBase, list) };
            assert!(!ptr::eq(ptr, obj));
        });
    }
}

pub fn insert(object_tpye: u8, node: &mut ListHead) {
    let process = Kprocess::get_process();
    process.insert(object_tpye, node);
}

#[cfg(feature = "RT_USING_DEBUG")]
pub fn object_addr_detect(object_tpye: u8, ptr: &mut KObjectBase) {
    let process = Kprocess::get_process();
    process.addr_detect(object_tpye, ptr);
}

/// TODO: remove this fuction
/// Find the kernel object by name
pub fn find_object(object_tpye: u8, name: *const i8) -> *const KObjectBase {
    if name.is_null() {
        return ptr::null_mut();
    }

    /* which is invoke in interrupt status */
    rt_bindings::rt_debug_not_in_interrupt!();

    let process = Kprocess::get_process();
    let list = process.get_object_list(object_tpye);
    let _ = process.lock.acquire();
    /* enter critical */
    rt_enter_critical();
    /* try to find object */
    crate::list_head_for_each!(node, list, {
        unsafe {
            let object = crate::list_head_entry!(node.as_ptr(), KObjectBase, list);
            if rt_strncmp(
                (*object).name.as_ptr() as *const ffi::c_char,
                name,
                NAME_MAX,
            ) == 0
            {
                /* leave critical */
                rt_exit_critical();
                return object;
            }
        }
    });
    /* leave critical */
    rt_exit_critical();

    ptr::null_mut()
}

pub fn get_objects_by_type(object_type: u8, objects: &mut [*mut KObjectBase]) -> usize {
    if object_type > ObjectClassType::ObjectClassUninit as u8
        && object_type < ObjectClassType::ObjectClassUnknown as u8
    {
        let mut count: usize = 0;
        let maxlen: usize = objects.len();
        let process = Kprocess::get_process();
        let list = process.get_object_list(object_type);
        let _ = process.lock.acquire();
        crate::list_head_for_each!(node, list, {
            let object = unsafe { crate::list_head_entry!(node.as_ptr(), KObjectBase, list) };
            objects[count] = object as *mut KObjectBase;
            count += 1;
            if count >= maxlen {
                break;
            }
        });
        count
    } else {
        0
    }
}

pub fn foreach<F>(callback: F, object_type: u8) -> Result<(), i32>
where
    F: Fn(&ListHead),
{
    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    process.lock.lock();
    crate::list_head_for_each!(node, list, {
        let _ = process.lock.unlock();
        callback(node);
        let _ = process.lock.lock();
    });
    let _ = process.lock.unlock();
    Ok(())
}

pub fn rt_foreach(
    callback: extern "C" fn(rt_bindings::rt_object_t, usize, *mut core::ffi::c_void),
    object_type: u8,
    args: *mut ffi::c_void,
) -> Result<(), i32> {
    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    let mut index: usize = 0;
    process.lock.lock();
    crate::list_head_for_each!(node, list, {
        let obj = unsafe { crate::list_head_entry!(node.as_ptr(), KObjectBase, list) };
        let _ = process.lock.unlock();
        callback(obj as rt_bindings::rt_object_t, index, args);
        let _ = process.lock.lock();
        index = index + 1;
    });
    let _ = process.lock.unlock();
    Ok(())
}

pub fn size(object_type: u8) -> usize {
    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    let _ = process.lock.acquire();
    let size = list.size();
    size
}

pub fn remove(object: &mut KObjectBase) {
    let process = Kprocess::get_process();
    let _ = process.lock.acquire();
    unsafe { Pin::new_unchecked(&mut object.list).remove() };
}
