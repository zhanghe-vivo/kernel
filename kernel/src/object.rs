use crate::{
    allocator::free,
    process::{foreach, insert, remove, Kprocess},
    sync::{
        condvar::CondVar,
        event::Event,
        lock::{mutex::Mutex, rwlock::RwLock},
        mailbox::Mailbox,
        message_queue::MessageQueue,
        semaphore::Semaphore,
    },
    thread::Thread,
    timer::Timer,
};
use alloc::boxed::Box;
use bluekernel_infra::list::doubly_linked_list::{LinkedListNode, ListHead};
use core::{fmt::Debug, mem, pin::Pin};
use pinned_init::{pin_data, pin_init, PinInit};

/// Base kernel Object
#[pin_data]
#[derive(Debug)]
#[repr(C)]
pub struct KObjectBase {
    /// name of kernel object
    pub name: [i8; NAME_MAX],
    /// type of kernel object
    pub type_: ObjectClassType,
    /// list node of kernel object
    #[pin]
    pub list: LinkedListNode,
}

impl KObjectBase {
    pub fn init(&mut self, type_: ObjectClassType, name: *const i8) {
        self.init_internal(type_.with_static(), name);
    }

    pub(crate) fn init_dyn(&mut self, type_: ObjectClassType, name: *const i8) {
        self.init_internal(type_, name);
    }

    pub(crate) fn init_internal(&mut self, type_: ObjectClassType, name: *const i8) {
        self.type_ = type_;
        unsafe {
            let len = core::cmp::min(
                NAME_MAX - 1,
                core::ffi::CStr::from_ptr(name).to_bytes().len(),
            );
            core::ptr::copy_nonoverlapping(name, self.name.as_mut_ptr(), len);
            self.name[len] = 0;
            Pin::new_unchecked(&mut self.list).reset();
        }

        if type_.without_static() != ObjectClassType::ObjectClassProcess {
            insert(type_, &mut self.list);
        }
    }

    /// This new function called by rust
    pub(crate) fn new(type_: ObjectClassType, name: &str) -> impl PinInit<Self> {
        let mut name_array = [0i8; NAME_MAX];
        let bytes = name.as_bytes();
        let copy_len = core::cmp::min(bytes.len(), NAME_MAX - 1);
        for i in 0..copy_len {
            name_array[i] = bytes[i] as i8;
        }
        name_array[copy_len] = 0;

        pin_init!(Self {
            name: name_array,
            type_: type_,
            list <- LinkedListNode::new(),
        })
    }

    /// This new function called by c
    pub fn new_raw(type_: ObjectClassType, name: *const i8) -> *mut KObjectBase {
        let object_size = ObjectClassType::get_object_size(type_);
        crate::debug_not_in_interrupt!();

        let object = Box::<[u8]>::new_zeroed_slice(object_size);
        let object_mut = Box::leak(object).as_mut_ptr().cast::<KObjectBase>();
        unsafe {
            (*object_mut).init_internal(type_, name);
        }
        object_mut
    }

    pub fn detach(&mut self) {
        remove(self);
        self.type_ = ObjectClassType::ObjectClassUninit;
    }

    pub fn delete(&mut self) {
        assert!(!self.type_.is_static());
        remove(self);
        self.type_ = ObjectClassType::ObjectClassUninit;
        free(self as *mut _ as *mut u8);
    }
}

pub const NAME_MAX: usize = 8;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectClassType {
    ObjectClassUninit = 0,
    //< The object is a process.
    ObjectClassProcess,
    //< The object is a thread.
    ObjectClassThread,
    //< The object is a semaphore.
    #[cfg(feature = "semaphore")]
    ObjectClassSemaphore,
    //< The object is a mutex.
    #[cfg(feature = "mutex")]
    ObjectClassMutex,
    //< The object is a condition variable.
    #[cfg(feature = "condvar")]
    ObjectClassCondVar,
    //< The object is a RwLock.
    #[cfg(feature = "rwlock")]
    ObjectClassRwLock,
    //< The object is an event.
    #[cfg(feature = "event")]
    ObjectClassEvent,
    //< The object is a mailbox.
    #[cfg(feature = "mailbox")]
    ObjectClassMailBox,
    //< The object is a message queue.
    #[cfg(feature = "messagequeue")]
    ObjectClassMessageQueue,
    //< The object is a memory heap.
    #[cfg(feature = "memheap")]
    ObjectClassMemHeap,
    //< The object is a memory pool.
    #[cfg(feature = "mempool")]
    ObjectClassMemPool,
    //< The object is a timer.
    ObjectClassTimer,
    //< The object is memory.
    #[cfg(feature = "heap")]
    ObjectClassMemory,
    ObjectClassUnknown,

    // The object is a static object (bit flag)
    Static = 0x80,
}

/// Common interface of a kernel object.
pub trait KernelObject {
    /// Get the name of the type of the kernel object.
    fn type_name(&self) -> ObjectClassType;
    /// Get kernel object's name.
    fn name(&self) -> *const i8;
    /// Set kernel object's name.
    fn set_name(&mut self, name: *const i8);
    /// Checks whether the kernel object is a static object.
    fn is_static_kobject(&self) -> bool;
    /// This function is used to iterate all kernel objects.
    fn foreach<F>(callback: F, type_: ObjectClassType) -> Result<(), i32>
    where
        F: Fn(&ListHead),
        Self: Sized;
    /// Get the kernel object info.
    fn get_info<FF, F>(
        callback_forword: FF,
        callback: F,
        type_: ObjectClassType,
    ) -> Result<(), i32>
    where
        FF: Fn(),
        F: Fn(&ListHead),
        Self: Sized;
}

impl KernelObject for KObjectBase {
    fn type_name(&self) -> ObjectClassType {
        self.type_.without_static()
    }

    fn name(&self) -> *const i8 {
        self.name.as_ptr()
    }

    fn set_name(&mut self, name: *const i8) {
        assert!(!name.is_null());
        unsafe {
            let len = core::cmp::min(
                NAME_MAX - 1,
                core::ffi::CStr::from_ptr(name).to_bytes().len(),
            );
            core::ptr::copy_nonoverlapping(name, self.name.as_mut_ptr(), len);
            self.name[len] = 0;
        }
    }

    fn is_static_kobject(&self) -> bool {
        self.type_.is_static()
    }

    fn foreach<F>(callback: F, type_: ObjectClassType) -> Result<(), i32>
    where
        F: Fn(&ListHead),
        Self: Sized,
    {
        foreach(callback, type_)
    }

    fn get_info<FF, F>(callback_forword: FF, callback: F, type_: ObjectClassType) -> Result<(), i32>
    where
        FF: Fn(),
        F: Fn(&ListHead),
        Self: Sized,
    {
        callback_forword();
        Self::foreach(callback, type_)
    }
}

impl ObjectClassType {
    pub fn with_static(self) -> Self {
        Self::from_u8(self as u8 | Self::Static as u8)
    }

    pub fn without_static(self) -> Self {
        Self::from_u8(self as u8 & (Self::Static as u8 ^ 0xFF))
    }

    pub fn is_static(self) -> bool {
        (self as u8 & Self::Static as u8) != 0
    }

    pub fn from_u8(value: u8) -> Self {
        // transmute will not work for release mode
        let base_type = value & (Self::Static as u8 ^ 0xFF);
        let is_static = (value & Self::Static as u8) != 0;

        let mut result = match base_type {
            // 基本类型
            x if x == Self::ObjectClassUninit as u8 => Self::ObjectClassUninit,
            x if x == Self::ObjectClassProcess as u8 => Self::ObjectClassProcess,
            x if x == Self::ObjectClassThread as u8 => Self::ObjectClassThread,
            #[cfg(feature = "semaphore")]
            x if x == Self::ObjectClassSemaphore as u8 => Self::ObjectClassSemaphore,
            #[cfg(feature = "mutex")]
            x if x == Self::ObjectClassMutex as u8 => Self::ObjectClassMutex,
            #[cfg(feature = "condvar")]
            x if x == Self::ObjectClassCondVar as u8 => Self::ObjectClassCondVar,
            #[cfg(feature = "rwlock")]
            x if x == Self::ObjectClassRwLock as u8 => Self::ObjectClassRwLock,
            #[cfg(feature = "event")]
            x if x == Self::ObjectClassEvent as u8 => Self::ObjectClassEvent,
            #[cfg(feature = "mailbox")]
            x if x == Self::ObjectClassMailBox as u8 => Self::ObjectClassMailBox,
            #[cfg(feature = "messagequeue")]
            x if x == Self::ObjectClassMessageQueue as u8 => Self::ObjectClassMessageQueue,
            #[cfg(feature = "memheap")]
            x if x == Self::ObjectClassMemHeap as u8 => Self::ObjectClassMemHeap,
            #[cfg(feature = "mempool")]
            x if x == Self::ObjectClassMemPool as u8 => Self::ObjectClassMemPool,
            x if x == Self::ObjectClassTimer as u8 => Self::ObjectClassTimer,
            #[cfg(feature = "heap")]
            x if x == Self::ObjectClassMemory as u8 => Self::ObjectClassMemory,
            _ => Self::ObjectClassUnknown,
        };

        if is_static {
            unsafe { *(&mut result as *mut Self as *mut u8) |= Self::Static as u8 };
        }

        result
    }

    pub fn try_from_u8(value: u8) -> Option<Self> {
        let base_type = value & (Self::Static as u8 ^ 0xFF);

        if base_type < Self::ObjectClassUnknown as u8 && base_type != 0 {
            Some(Self::from_u8(value))
        } else {
            None
        }
    }

    fn get_object_size(obj_type: Self) -> usize {
        match obj_type.without_static() {
            Self::ObjectClassProcess => mem::size_of::<Kprocess>(),
            Self::ObjectClassThread => mem::size_of::<Thread>(),
            #[cfg(feature = "semaphore")]
            Self::ObjectClassSemaphore => mem::size_of::<Semaphore>(),
            #[cfg(feature = "mutex")]
            Self::ObjectClassMutex => mem::size_of::<Mutex>(),
            #[cfg(feature = "condvar")]
            Self::ObjectClassCondVar => mem::size_of::<CondVar>(),
            #[cfg(feature = "rwlock")]
            Self::ObjectClassRwLock => mem::size_of::<RwLock>(),
            #[cfg(feature = "event")]
            Self::ObjectClassEvent => mem::size_of::<Event>(),
            #[cfg(feature = "mailbox")]
            Self::ObjectClassMailBox => mem::size_of::<Mailbox>(),
            #[cfg(feature = "messagequeue")]
            Self::ObjectClassMessageQueue => mem::size_of::<MessageQueue>(),
            #[cfg(feature = "memheap")]
            Self::ObjectClassMemHeap => mem::size_of::<Memheap>(),
            #[cfg(feature = "mempool")]
            Self::ObjectClassMemPool => mem::size_of::<Mempool>(),
            Self::ObjectClassTimer => mem::size_of::<Timer>(),
            #[cfg(feature = "heap")]
            Self::ObjectClassMemory => mem::size_of::<Memory>(),
            _ => unreachable!("not a valid kobject type!"),
        }
    }
}

impl From<u8> for ObjectClassType {
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<ObjectClassType> for u8 {
    fn from(value: ObjectClassType) -> Self {
        value as u8
    }
}

#[macro_export]
macro_rules! impl_kobject {
    ($class:ident $( $fn:tt )*) => {
        impl $crate::object::KernelObject for $class {
            fn type_name(&self) -> ObjectClassType{
                self.parent.type_name()
            }
            fn name(&self) -> *const i8{
                self.parent.name()
            }
            fn set_name(&mut self, name: *const i8){
                self.parent.set_name(name);
            }
            fn is_static_kobject(&self) -> bool{
                self.parent.is_static_kobject()
            }
            fn foreach<F>(callback: F, type_: ObjectClassType) -> Result<(), i32>
            where
                F: Fn(&bluekernel_infra::list::doubly_linked_list::ListHead),
                Self: Sized
            {
                KObjectBase::foreach(callback, type_)
            }
            fn get_info<FF,F>(callback_forword: FF, callback: F, type_: ObjectClassType) -> Result<(), i32>
            where
                FF: Fn(),
                F: Fn(&bluekernel_infra::list::doubly_linked_list::ListHead),
                Self: Sized
            {
                KObjectBase::get_info(callback_forword,callback, type_)
            }
            $( $fn )*
        }
    };
}

//TODO: add mempool
pub struct Mempool;

//TODO: add memheap
pub struct Memheap;

//TODO: add memory
#[repr(C)]
pub struct Memory {
    #[doc = "< inherit from rt_object"]
    pub parent: KObjectBase,
    #[doc = "< Memory management algorithm name"]
    pub algorithm: *const core::ffi::c_char,
    #[doc = "< memory start address"]
    pub address: usize,
    #[doc = "< memory size"]
    pub total: usize,
    #[doc = "< size used"]
    pub used: usize,
    #[doc = "< maximum usage"]
    pub max: usize,
}

/// bindgen for ObjectClassType
#[no_mangle]
pub extern "C" fn bindgen_object_class_type(_obj: ObjectClassType) {
    0;
}

/// bindgen for BaseObject
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_base_object(_obj: KObjectBase) {
    0;
}
