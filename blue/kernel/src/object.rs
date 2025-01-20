use crate::{
    allocator::{free, malloc},
    klibc::{memset, strncpy},
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
use blue_infra::list::doubly_linked_list::ListHead;
use core::{fmt::Debug, mem, ptr};
use pinned_init::{pin_data, pin_init, PinInit};

/// Base kernel Object
#[pin_data]
#[derive(Debug)]
#[repr(C)]
pub struct KObjectBase {
    /// TODO: change type to String
    /// name of kernel object
    pub name: [i8; NAME_MAX],
    /// type of kernel object
    pub type_: u8,
    /// list node of kernel object
    #[pin]
    pub list: ListHead,
}

impl KObjectBase {
    pub fn init(&mut self, type_: u8, name: *const i8) {
        self.init_internal(type_ | OBJECT_CLASS_STATIC, name);
    }

    pub(crate) fn init_dyn(&mut self, type_: u8, name: *const i8) {
        self.init_internal(type_, name);
    }

    pub(crate) fn init_internal(&mut self, type_: u8, name: *const i8) {
        self.type_ = type_;
        unsafe {
            strncpy(self.name.as_mut_ptr(), name, (NAME_MAX - 1) as usize);
        }

        if type_ & (!OBJECT_CLASS_STATIC) != ObjectClassType::ObjectClassProcess as u8 {
            insert(type_, &mut self.list);
        }
    }

    /// This new function called by rust
    pub(crate) fn new(type_: u8, name: [i8; NAME_MAX]) -> impl PinInit<Self> {
        pin_init!(Self {
            name: name,
            type_: type_,
            list <- ListHead::new(),
        })
    }

    /// This new function called by c
    pub fn new_raw(type_: u8, name: *const i8) -> *mut KObjectBase {
        let object_size = ObjectClassType::get_object_size(type_ as u8);

        crate::debug_not_in_interrupt!();

        let object = malloc(object_size) as *mut KObjectBase;
        if object.is_null() {
            return ptr::null_mut();
        }
        unsafe { memset(object as *mut u8, 0x0, object_size) };

        let obj_ref = unsafe { &mut *object };
        obj_ref.init_internal(type_, name);
        object
    }

    pub fn detach(&mut self) {
        remove(self);
        self.type_ = ObjectClassType::ObjectClassUninit as u8;
    }

    pub fn delete(&mut self) {
        assert!((self.type_ & OBJECT_CLASS_STATIC) == 0);
        remove(self);
        self.type_ = ObjectClassType::ObjectClassUninit as u8;
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
    //< The object is a device.
    ObjectClassDevice,
    //< The object is a timer.
    ObjectClassTimer,
    //< The object is memory.
    #[cfg(feature = "heap")]
    ObjectClassMemory,
    ObjectClassUnknown,
}

/// Common interface of a kernel object.
pub trait KernelObject {
    /// Get the name of the type of the kernel object.
    fn type_name(&self) -> u8;
    /// Get kernel object's name.
    fn name(&self) -> *const i8;
    /// Set kernel object's name.
    fn set_name(&mut self, name: *const i8);
    /// Checks whether the kernel object is a static object.
    fn is_static_kobject(&self) -> bool;
    /// This function is used to iterate all kernel objects.
    fn foreach<F>(callback: F, type_: u8) -> Result<(), i32>
    where
        F: Fn(&ListHead),
        Self: Sized;
    /// Get the kernel object info.
    fn get_info<FF, F>(callback_forword: FF, callback: F, type_: u8) -> Result<(), i32>
    where
        FF: Fn(),
        F: Fn(&ListHead),
        Self: Sized;
}

impl KernelObject for KObjectBase {
    fn type_name(&self) -> u8 {
        self.type_ & (!OBJECT_CLASS_STATIC)
    }

    fn name(&self) -> *const i8 {
        self.name.as_ptr()
    }

    fn set_name(&mut self, name: *const i8) {
        assert!(!name.is_null());
        unsafe {
            strncpy(self.name.as_mut_ptr(), name, (NAME_MAX - 1) as usize);
        }
    }

    fn is_static_kobject(&self) -> bool {
        let obj_type = self.type_;
        if (obj_type & OBJECT_CLASS_STATIC) != 0 {
            return true;
        }
        return false;
    }

    fn foreach<F>(callback: F, type_: u8) -> Result<(), i32>
    where
        F: Fn(&ListHead),
        Self: Sized,
    {
        foreach(callback, type_)
    }

    fn get_info<FF, F>(callback_forword: FF, callback: F, type_: u8) -> Result<(), i32>
    where
        FF: Fn(),
        F: Fn(&ListHead),
        Self: Sized,
    {
        callback_forword();
        Self::foreach(callback, type_)
    }
}

/// The object is a static object.
pub(crate) const OBJECT_CLASS_STATIC: u8 = 0x80;

impl ObjectClassType {
    // 为枚举类型添加方法
    fn get_object_size(index: u8) -> usize {
        match index {
            //< The object is a process.
            x if x == Self::ObjectClassProcess as u8 => mem::size_of::<Kprocess>(),
            //< The object is a thread.
            x if x == Self::ObjectClassThread as u8 => mem::size_of::<Thread>(),
            //< The object is a semaphore.
            #[cfg(feature = "semaphore")]
            x if x == Self::ObjectClassSemaphore as u8 => mem::size_of::<Semaphore>(),
            //< The object is a mutex.
            #[cfg(feature = "mutex")]
            x if x == Self::ObjectClassMutex as u8 => mem::size_of::<Mutex>(),
            //< The object is a condition variable.
            #[cfg(feature = "condvar")]
            x if x == Self::ObjectClassCondVar as u8 => mem::size_of::<CondVar>(),
            //< The object is a RwLock.
            #[cfg(feature = "rwlock")]
            x if x == Self::ObjectClassRwLock as u8 => mem::size_of::<RwLock>(),
            //< The object is an event.
            #[cfg(feature = "event")]
            x if x == Self::ObjectClassEvent as u8 => mem::size_of::<Event>(),
            //< The object is a mailbox.
            #[cfg(feature = "mailbox")]
            x if x == Self::ObjectClassMailBox as u8 => mem::size_of::<Mailbox>(),
            //< The object is a message queue.
            #[cfg(feature = "messagequeue")]
            x if x == Self::ObjectClassMessageQueue as u8 => mem::size_of::<MessageQueue>(),
            //< The object is a memory heap.
            #[cfg(feature = "memheap")]
            x if x == Self::ObjectClassMemHeap as u8 => mem::size_of::<Memheap>(),
            //< The object is a memory pool.
            #[cfg(feature = "mempool")]
            x if x == Self::ObjectClassMemPool as u8 => mem::size_of::<Mempool>(),
            //< The object is a device.
            x if x == Self::ObjectClassDevice as u8 => mem::size_of::<Device>(),
            //< The object is a timer.
            x if x == Self::ObjectClassTimer as u8 => mem::size_of::<Timer>(),
            //< The object is memory.
            #[cfg(feature = "heap")]
            x if x == Self::ObjectClassMemory as u8 => mem::size_of::<Memory>(),
            _ => unreachable!("not a static kobject type!"),
        }
    }
}

#[macro_export]
macro_rules! impl_kobject {
    ($class:ident $( $fn:tt )*) => {
        impl $crate::object::KernelObject for $class {
            fn type_name(&self) -> u8{
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
            fn foreach<F>(callback: F, type_: u8) -> Result<(), i32>
            where
                F: Fn(&ListHead),
                Self: Sized
            {
                KObjectBase::foreach(callback, type_)
            }
            fn get_info<FF,F>(callback_forword: FF, callback: F, type_: u8) -> Result<(), i32>
            where
                FF: Fn(),
                F: Fn(&ListHead),
                Self: Sized
            {
                KObjectBase::get_info(callback_forword,callback, type_)
            }
            $( $fn )*
        }
    };
}

#[macro_export]
macro_rules! format_name {
    ($name:expr,$width:expr) => {{
        use crate::str::CStr;
        let name_cstr = CStr::from_char_ptr($name);
        match name_cstr.to_str() {
            Ok(name) => {
                crate::print!("{:<1$}", name, $width);
            }
            Err(_) => {
                crate::println!("Error when converting C string to UTF-8");
            }
        }
    }};
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

//TODO: add device
#[repr(C)]
pub struct Device {
    #[doc = "< inherit from rt_object"]
    pub parent: KObjectBase,
    #[doc = "< device type"]
    pub type_: isize,
    #[doc = "< device flag"]
    pub flag: u16,
    #[doc = "< device open flag"]
    pub open_flag: u16,
    #[doc = "< reference count"]
    pub ref_count: u8,
    #[doc = "< 0 - 255"]
    pub device_id: u8,
    pub rx_indicate:
        ::core::option::Option<unsafe extern "C" fn(dev: *mut Device, size: usize) -> usize>,
    pub tx_complete: ::core::option::Option<
        unsafe extern "C" fn(dev: *mut Device, buffer: *mut core::ffi::c_void) -> usize,
    >,
    pub init: ::core::option::Option<unsafe extern "C" fn(dev: *mut Device) -> usize>,
    pub open: ::core::option::Option<unsafe extern "C" fn(dev: *mut Device, oflag: u16) -> usize>,
    pub close: ::core::option::Option<unsafe extern "C" fn(dev: *mut Device) -> usize>,
    pub read: ::core::option::Option<
        unsafe extern "C" fn(
            dev: *mut Device,
            pos: usize,
            buffer: *mut core::ffi::c_void,
            size: usize,
        ) -> usize,
    >,
    pub write: ::core::option::Option<
        unsafe extern "C" fn(
            dev: *mut Device,
            pos: usize,
            buffer: *const core::ffi::c_void,
            size: usize,
        ) -> usize,
    >,
    pub control: ::core::option::Option<
        unsafe extern "C" fn(
            dev: *mut Device,
            cmd: core::ffi::c_int,
            args: *mut core::ffi::c_void,
        ) -> usize,
    >,
    #[doc = "< device private data"]
    pub user_data: *mut core::ffi::c_void,
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
