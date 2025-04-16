use crate::{
    cpu::Cpu,
    object::{KObjectBase, ObjectClassType, NAME_MAX},
    static_init::UnsafeStaticInit,
    sync::RawSpin,
};
use bluekernel_infra::{
    klibc,
    list::doubly_linked_list::{LinkedListNode, ListHead},
};

use core::{
    ffi,
    pin::Pin,
    ptr::{self, addr_of_mut},
};
use pinned_init::{pin_data, pin_init_from_closure, PinInit};

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
    sibling: LinkedListNode,
    ///not use yet
    #[pin]
    children: LinkedListNode,
    #[pin]
    threads: ListHead,
    #[cfg(feature = "semaphore")]
    #[pin]
    semaphore: ListHead,
    #[cfg(feature = "mutex")]
    #[pin]
    mutex: ListHead,
    #[cfg(feature = "rwlock")]
    #[pin]
    rwlock: ListHead,
    #[cfg(feature = "event")]
    #[pin]
    event: ListHead,
    #[cfg(feature = "condvar")]
    #[pin]
    condvar: ListHead,
    #[cfg(feature = "mailbox")]
    #[pin]
    mailbox: ListHead,
    #[cfg(feature = "messagequeue")]
    #[pin]
    msgqueue: ListHead,
    #[cfg(feature = "memheap")]
    #[pin]
    memheap: ListHead,
    #[cfg(feature = "mempool")]
    #[pin]
    mempool: ListHead,
    #[pin]
    timer: ListHead,
    #[cfg(feature = "heap")]
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
                ObjectClassType::ObjectClassProcess,
                crate::c_str!("kprocess").as_ptr() as *const i8,
            );
            let _ =
                LinkedListNode::new().__pinned_init(&mut cur_ref.sibling as *mut LinkedListNode);
            let _ =
                LinkedListNode::new().__pinned_init(&mut cur_ref.children as *mut LinkedListNode);
            let _ = ListHead::new().__pinned_init(&mut cur_ref.threads as *mut ListHead);
            #[cfg(feature = "semaphore")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.semaphore as *mut ListHead);
            #[cfg(feature = "mutex")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.mutex as *mut ListHead);
            #[cfg(feature = "rwlock")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.rwlock as *mut ListHead);
            #[cfg(feature = "event")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.event as *mut ListHead);
            #[cfg(feature = "condvar")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.condvar as *mut ListHead);
            #[cfg(feature = "mailbox")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.mailbox as *mut ListHead);
            #[cfg(feature = "messagequeue")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.msgqueue as *mut ListHead);
            #[cfg(feature = "memheap")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.memheap as *mut ListHead);
            #[cfg(feature = "mempool")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.mempool as *mut ListHead);
            let _ = ListHead::new().__pinned_init(&mut cur_ref.timer as *mut ListHead);
            #[cfg(feature = "heap")]
            let _ = ListHead::new().__pinned_init(&mut cur_ref.memory as *mut ListHead);
            cur_ref.pid = 0;
            cur_ref.lock = RawSpin::new();
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[inline]
    fn get_object_list_mut(&self, object_type: ObjectClassType) -> &mut ListHead {
        let process = Kprocess::get_process();
        let _ = process.lock.acquire();
        match object_type.without_static() {
            ObjectClassType::ObjectClassThread => &mut process.threads,
            #[cfg(feature = "semaphore")]
            ObjectClassType::ObjectClassSemaphore => &mut process.semaphore,
            #[cfg(feature = "mutex")]
            ObjectClassType::ObjectClassMutex => &mut process.mutex,
            #[cfg(feature = "rwlock")]
            ObjectClassType::ObjectClassRwLock => &mut process.rwlock,
            #[cfg(feature = "event")]
            ObjectClassType::ObjectClassEvent => &mut process.event,
            #[cfg(feature = "condvar")]
            ObjectClassType::ObjectClassCondVar => &mut process.condvar,
            #[cfg(feature = "mailbox")]
            ObjectClassType::ObjectClassMailBox => &mut process.mailbox,
            #[cfg(feature = "messagequeue")]
            ObjectClassType::ObjectClassMessageQueue => &mut process.msgqueue,
            #[cfg(feature = "memheap")]
            ObjectClassType::ObjectClassMemHeap => &mut process.memheap,
            #[cfg(feature = "mempool")]
            ObjectClassType::ObjectClassMemPool => &mut process.mempool,
            ObjectClassType::ObjectClassTimer => &mut process.timer,
            #[cfg(feature = "heap")]
            ObjectClassType::ObjectClassMemory => &mut process.memory,
            _ => unreachable!("not a kernel object type!"),
        }
    }

    #[inline]
    fn get_object_list(&self, object_type: ObjectClassType) -> &ListHead {
        let process = Kprocess::get_process();
        let _ = process.lock.acquire();
        match object_type.without_static() {
            ObjectClassType::ObjectClassThread => &process.threads,
            #[cfg(feature = "semaphore")]
            ObjectClassType::ObjectClassSemaphore => &process.semaphore,
            #[cfg(feature = "mutex")]
            ObjectClassType::ObjectClassMutex => &process.mutex,
            #[cfg(feature = "rwlock")]
            ObjectClassType::ObjectClassRwLock => &process.rwlock,
            #[cfg(feature = "event")]
            ObjectClassType::ObjectClassEvent => &process.event,
            #[cfg(feature = "condvar")]
            ObjectClassType::ObjectClassCondVar => &process.condvar,
            #[cfg(feature = "mailbox")]
            ObjectClassType::ObjectClassMailBox => &process.mailbox,
            #[cfg(feature = "messagequeue")]
            ObjectClassType::ObjectClassMessageQueue => &process.msgqueue,
            #[cfg(feature = "memheap")]
            ObjectClassType::ObjectClassMemHeap => &process.memheap,
            #[cfg(feature = "mempool")]
            ObjectClassType::ObjectClassMemPool => &process.mempool,
            ObjectClassType::ObjectClassTimer => &process.timer,
            #[cfg(feature = "heap")]
            ObjectClassType::ObjectClassMemory => &process.memory,
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

    fn insert(&mut self, object_type: ObjectClassType, node: &mut ListHead) {
        let list = self.get_object_list_mut(object_type);
        let _ = self.lock.acquire();

        unsafe {
            Pin::new_unchecked(list).push_back(Pin::new_unchecked(node));
        }
    }

    #[cfg(feature = "debugging_object")]
    fn addr_detect(&mut self, object_type: ObjectClassType, ptr: &mut KObjectBase) {
        let list = self.get_object_list(object_type);
        let _ = self.lock.acquire();
        crate::doubly_linked_list_for_each!(node, list, {
            let obj = unsafe { crate::list_head_entry!(node.as_ptr(), KObjectBase, list) };
            assert!(!ptr::eq(ptr, obj));
        });
    }
}

pub fn insert(object_type: ObjectClassType, node: &mut ListHead) {
    let process = Kprocess::get_process();
    process.insert(object_type, node);
}

#[cfg(feature = "debugging_object")]
pub fn object_addr_detect(object_type: ObjectClassType, ptr: &mut KObjectBase) {
    let process = Kprocess::get_process();
    process.addr_detect(object_type, ptr);
}

/// TODO: remove this fuction
/// Find the kernel object by name
pub fn find_object(object_type: ObjectClassType, name: *const i8) -> *const KObjectBase {
    if name.is_null() {
        return ptr::null_mut();
    }

    /* which is invoke in interrupt status */
    crate::debug_not_in_interrupt!();

    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    let _ = process.lock.acquire();
    /* enter critical */
    Cpu::get_current_scheduler().preempt_disable();
    /* try to find object */
    crate::doubly_linked_list_for_each!(node, list, {
        unsafe {
            let object = crate::list_head_entry!(node.as_ptr(), KObjectBase, list);
            if klibc::strncmp(
                (*object).name.as_ptr() as *const ffi::c_char,
                name,
                NAME_MAX,
            ) == 0
            {
                /* leave critical */
                Cpu::get_current_scheduler().preempt_enable();
                return object;
            }
        }
    });
    /* leave critical */
    Cpu::get_current_scheduler().preempt_enable();

    ptr::null_mut()
}

pub fn get_objects_by_type(
    object_type: ObjectClassType,
    objects: &mut [*mut KObjectBase],
) -> usize {
    if object_type > ObjectClassType::ObjectClassUninit
        && object_type < ObjectClassType::ObjectClassUnknown
    {
        let mut count: usize = 0;
        let maxlen: usize = objects.len();
        let process = Kprocess::get_process();
        let list = process.get_object_list(object_type);
        let _ = process.lock.acquire();
        crate::doubly_linked_list_for_each!(node, list, {
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

pub fn foreach<F>(callback: F, object_type: ObjectClassType) -> Result<(), i32>
where
    F: Fn(&ListHead),
{
    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    process.lock.lock();
    crate::doubly_linked_list_for_each!(node, list, {
        let _ = process.lock.unlock();
        callback(node);
        let _ = process.lock.lock();
    });
    let _ = process.lock.unlock();
    Ok(())
}

pub fn bindings_foreach(
    callback: extern "C" fn(*mut KObjectBase, usize, *mut core::ffi::c_void),
    object_type: ObjectClassType,
    args: *mut ffi::c_void,
) -> Result<(), i32> {
    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    let mut index: usize = 0;
    process.lock.lock();
    crate::doubly_linked_list_for_each!(node, list, {
        let obj = unsafe { crate::list_head_entry!(node.as_ptr(), KObjectBase, list) };
        let _ = process.lock.unlock();
        callback(obj as *mut KObjectBase, index, args);
        let _ = process.lock.lock();
        index = index + 1;
    });
    let _ = process.lock.unlock();
    Ok(())
}

pub fn get_object_size(object_type: ObjectClassType) -> usize {
    let process = Kprocess::get_process();
    let list = process.get_object_list(object_type);
    let _ = process.lock.acquire();
    let size = list.size();
    size
}

pub fn remove(object: &mut KObjectBase) {
    let process = Kprocess::get_process();
    let _ = process.lock.acquire();
    unsafe { Pin::new_unchecked(&mut object.list).remove_from_list() };
}
