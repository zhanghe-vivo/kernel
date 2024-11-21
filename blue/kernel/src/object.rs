use crate::{
    allocator::{rt_free, rt_malloc},
    klibc::{rt_memset, rt_strncpy},
    print, println,
    process::*,
    sync::event::RtEvent,
    sync::lock::mutex::RtMutex,
    sync::mailbox::RtMailbox,
    sync::message_queue::RtMessageQueue,
    sync::semaphore::RtSemaphore,
    thread::RtThread,
    *,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi, fmt::Debug, mem, ptr, slice};
use {
    downcast_rs::{impl_downcast, Downcast},
    pinned_init::*,
    rt_bindings::*,
};

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
    pub(crate) fn init(&mut self, type_: u8, name: *const i8) {
        self.init_internal(type_ | OBJECT_CLASS_STATIC, name);
    }

    pub(crate) fn init_internal(&mut self, type_: u8, name: *const i8) {
        self.type_ = type_;
        unsafe {
            rt_strncpy(self.name.as_mut_ptr(), name, (NAME_MAX - 1) as usize);
            rt_bindings::rt_object_hook_call!(
                OBJECT_ATTACH_HOOK,
                self as *const _ as *const rt_object
            );
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
    pub(crate) fn new_raw(type_: u8, name: *const i8) -> *mut KObjectBase {
        use core::ffi::c_void;
        let object_size = ObjectClassType::get_object_size(type_ as u8);

        rt_bindings::rt_debug_not_in_interrupt!();

        let object = unsafe { rt_malloc(object_size) as *mut KObjectBase };
        if object.is_null() {
            return ptr::null_mut();
        }
        unsafe {
            rt_memset(object as *mut c_void, 0x0, object_size);
        }

        let obj_ref = unsafe { &mut *object };
        obj_ref.init_internal(type_, name);
        object
    }

    pub(crate) fn detach(&mut self) {
        unsafe {
            rt_bindings::rt_object_hook_call!(
                OBJECT_DETACH_HOOK,
                self as *const _ as *const rt_object
            )
        };
        remove(self);
        self.type_ = ObjectClassType::ObjectClassUninit as u8;
    }

    pub(crate) fn delete(&mut self) {
        assert!((self.type_ & OBJECT_CLASS_STATIC) == 0);
        unsafe {
            rt_bindings::rt_object_hook_call!(
                OBJECT_DETACH_HOOK,
                self as *const _ as *const rt_object
            )
        };
        remove(self);
        self.type_ = ObjectClassType::ObjectClassUninit as u8;
        unsafe {
            rt_free(self as *mut _ as *mut ffi::c_void);
        }
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
    #[cfg(feature = "RT_USING_SEMAPHORE")]
    ObjectClassSemaphore,
    //< The object is a mutex.
    #[cfg(feature = "RT_USING_MUTEX")]
    ObjectClassMutex,
    //< The object is an event.
    #[cfg(feature = "RT_USING_EVENT")]
    ObjectClassEvent,
    //< The object is a mailbox.
    #[cfg(feature = "RT_USING_MAILBOX")]
    ObjectClassMailBox,
    //< The object is a message queue.
    #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
    ObjectClassMessageQueue,
    //< The object is a memory heap.
    #[cfg(feature = "RT_USING_MEMHEAP")]
    ObjectClassMemHeap,
    //< The object is a memory pool.
    #[cfg(feature = "RT_USING_MEMPOOL")]
    ObjectClassMemPool,
    //< The object is a device.
    #[cfg(feature = "RT_USING_DEVICE")]
    ObjectClassDevice,
    //< The object is a timer.
    ObjectClassTimer,
    //< The object is memory.
    #[cfg(feature = "RT_USING_HEAP")]
    ObjectClassMemory,
    ObjectClassUnknown,
}

/// Common interface of a kernel object.
pub trait KernelObject: Downcast {
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

impl_downcast!(KernelObject);

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
            rt_strncpy(self.name.as_mut_ptr(), name, (NAME_MAX - 1) as usize);
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
            x if x == Self::ObjectClassThread as u8 => mem::size_of::<RtThread>(),
            //< The object is a semaphore.
            #[cfg(feature = "RT_USING_SEMAPHORE")]
            x if x == Self::ObjectClassSemaphore as u8 => mem::size_of::<RtSemaphore>(),
            //< The object is a mutex.
            #[cfg(feature = "RT_USING_MUTEX")]
            x if x == Self::ObjectClassMutex as u8 => mem::size_of::<RtMutex>(),
            //< The object is an event.
            #[cfg(feature = "RT_USING_EVENT")]
            x if x == Self::ObjectClassEvent as u8 => mem::size_of::<RtEvent>(),
            //< The object is a mailbox.
            #[cfg(feature = "RT_USING_MAILBOX")]
            x if x == Self::ObjectClassMailBox as u8 => mem::size_of::<RtMailbox>(),
            //< The object is a message queue.
            #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
            x if x == Self::ObjectClassMessageQueue as u8 => mem::size_of::<RtMessageQueue>(),
            //< The object is a memory heap.
            #[cfg(feature = "RT_USING_MEMHEAP")]
            x if x == Self::ObjectClassMemHeap as u8 => mem::size_of::<rt_memheap>(),
            //< The object is a memory pool.
            #[cfg(feature = "RT_USING_MEMPOOL")]
            x if x == Self::ObjectClassMemPool as u8 => mem::size_of::<rt_mempool>(),
            //< The object is a device.
            #[cfg(feature = "RT_USING_DEVICE")]
            x if x == Self::ObjectClassDevice as u8 => mem::size_of::<rt_device>(),
            //< The object is a timer.
            x if x == Self::ObjectClassTimer as u8 => mem::size_of::<rt_timer>(),
            //< The object is memory.
            #[cfg(feature = "RT_USING_HEAP")]
            x if x == Self::ObjectClassMemory as u8 => mem::size_of::<rt_memory>(),
            _ => unreachable!("not a static kobject type!"),
        }
    }
}

/// This function will return the length of object list in object container.
///
/// # Arguments
///
/// * `object_type` - The type of object, which can be RT_Object_Class_Thread, Semaphore, Mutex, etc.
///
/// # Returns
///
/// The length of object list.
#[no_mangle]
pub extern "C" fn rt_object_get_length(object_type: rt_object_class_type) -> usize {
    size(object_type as u8)
}

/// This function will copy the object pointer of the specified type, with the maximum size specified by maxlen.
///
/// # Arguments
///
/// * `object_type` - The type of object, which can be RT_Object_Class_Thread, Semaphore, Mutex, etc.
/// * `pointers` - The pointer will be saved to.
/// * `maxlen` - The maximum number of pointers that can be saved.
///
/// # Returns
///
/// The copied number of object pointers.
#[no_mangle]
pub unsafe extern "C" fn rt_object_get_pointers(
    object_type: rt_object_class_type,
    pointers: *mut rt_object_t,
    maxlen: usize,
) -> usize {
    if maxlen == 0 {
        return 0;
    }

    let object_slice: &mut [*mut KObjectBase] =
        slice::from_raw_parts_mut(pointers as *mut *mut KObjectBase, maxlen);
    get_objects_by_type(object_type as u8, object_slice)
}

/// This function will initialize an object and add it to object system
/// management.
///
/// # Arguments
///
/// * `object` - The specified object to be initialized.
/// * `type` - The object type.
/// * `name` - The object name. In the system, the object's name must be unique.
///
/// # Safety
///
/// This function is marked as unsafe because it performs low-level operations such as
/// modifying global state and working with raw pointers.
#[no_mangle]
pub extern "C" fn rt_object_init(
    object: *mut rt_object,
    type_: rt_object_class_type,
    name: *const ffi::c_char,
) {
    assert!(!object.is_null());
    let obj_ref = unsafe { &mut *(object as *mut KObjectBase) };
    #[cfg(feature = "RT_USING_DEBUG")]
    object_addr_detect(type_ as u8, obj_ref);
    // initialize object's parameters
    // set object type to static
    obj_ref.init(type_ as u8, name);
}

/// This function will detach a static object from the object system,
/// and the memory of the static object is not freed.
///
/// # Arguments
///
/// * `object` - The specified object to be detached.
#[no_mangle]
pub extern "C" fn rt_object_detach(object: *mut rt_object) {
    assert!(!object.is_null());
    let obj = unsafe { &mut *(object as *mut KObjectBase) };
    obj.detach();
}

/// This function will allocate an object from object system.
///
/// type is the type of object.
///
/// name is the object name. In system, the object's name must be unique.
///
/// Returns the allocated object.
#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_object_allocate(
    type_: rt_object_class_type,
    name: *const ffi::c_char,
) -> rt_object_t {
    KObjectBase::new_raw(type_ as u8, name) as rt_object_t
}

/// This function will delete an object and release object memory.
///
/// object is the specified object to be deleted.
#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_object_delete(object: rt_object_t) {
    // object check
    assert!(!object.is_null());
    let obj = unsafe { &mut *(object as *mut KObjectBase) };
    obj.delete();
}

/// This function will judge the object is system object or not.
///
/// Normally, the system object is a static object and the type
/// of object set to OBJECT_CLASS_STATIC.
///
/// # Arguments
///
/// * `object` - the specified object to be judged.
///
/// # Returns
///
/// `RT_TRUE` if a system object, `RT_FALSE` for others.
///
/// # Note
///
/// This function shall not be invoked in interrupt status.
#[no_mangle]
pub extern "C" fn rt_object_is_systemobject(object: rt_object_t) -> rt_bool_t {
    /* object check */
    assert!(!object.is_null());
    let obj = unsafe { &mut *(object as *mut KObjectBase) };
    let res = obj.is_static_kobject();

    if res {
        return RT_TRUE as ffi::c_int;
    }

    return RT_FALSE as ffi::c_int;
}

/// This function will return the type of object without
/// `OBJECT_CLASS_STATIC` flag.
///
/// # Arguments
///
/// * `object` - the specified object to get type.
///
/// # Returns
///
/// The type of object.
#[no_mangle]
pub extern "C" fn rt_object_get_type(object: rt_object_t) -> rt_uint8_t {
    /* object check */
    assert!(!object.is_null());
    let obj = unsafe { &mut *(object as *mut KObjectBase) };
    obj.type_name()
}

/// This function will find specified name object from object
/// container.
///
/// # Arguments
///
/// * `name` - the specified name of object.
/// * `type` - the type of object.
///
/// # Returns
///
/// The found object or `RT_NULL` if there is no this object
/// in object container.
///
/// # Note
///
/// This function shall not be invoked in interrupt status.
#[no_mangle]
pub extern "C" fn rt_object_find(name: *const ffi::c_char, type_: rt_uint8_t) -> rt_object_t {
    find_object(type_, name) as rt_object_t
}

/// This function will return the name of the specified object container
///
/// # Arguments
///
/// * `object` - the specified object to get name
/// * `name` - buffer to store the object name string
/// * `name_size` - maximum size of the buffer to store object name
///
/// # Returns
///
/// `-RT_EINVAL` if any parameter is invalid or `RT_EOK` if the operation is successfully executed
///
/// # Note
///
/// This function shall not be invoked in interrupt status
#[no_mangle]
pub extern "C" fn rt_object_get_name(
    object: rt_object_t,
    name: *const ffi::c_char,
    name_size: rt_uint8_t,
) -> rt_err_t {
    let mut result: rt_err_t = -(RT_EINVAL as i32);
    if !object.is_null() && !name.is_null() && name_size != 0 {
        let obj_name = (unsafe { *object }).name;
        unsafe { rt_strncpy(name as *mut _, obj_name.as_ptr(), name_size as usize) };
        result = RT_EOK as rt_err_t;
    }

    return result;
}

#[no_mangle]
pub extern "C" fn rt_object_for_each_callback(
    obj_type: u8,
    callback_fn: extern "C" fn(rt_object_t, usize, *mut ffi::c_void),
    args: *mut ffi::c_void,
) {
    rt_foreach(callback_fn, obj_type, args);
}

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut OBJECT_ATTACH_HOOK: Option<unsafe extern "C" fn(*const rt_object)> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut OBJECT_DETACH_HOOK: Option<unsafe extern "C" fn(*const rt_object)> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_object_trytake_hook: Option<unsafe extern "C" fn(*const rt_object)> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_object_take_hook: Option<unsafe extern "C" fn(*const rt_object)> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_object_put_hook: Option<unsafe extern "C" fn(*const rt_object)> = None;

/// This function sets a hook function, which will be invoked when an object attaches to the kernel object system.
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_object_attach_sethook(hook: unsafe extern "C" fn(*const rt_object)) {
    unsafe { OBJECT_ATTACH_HOOK = Some(hook) };
}

/// This function sets a hook function, which will be invoked when an object detaches from the kernel object system.
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_object_detach_sethook(hook: unsafe extern "C" fn(*const rt_object)) {
    unsafe { OBJECT_DETACH_HOOK = Some(hook) };
}

/// This function sets a hook function, which will be invoked when an object is taken from the kernel object system.
///
/// The object is taken means:
///   - semaphore: semaphore is taken by thread
///   - mutex: mutex is taken by thread
///   - event: event is received by thread
///   - mailbox: mail is received by thread
///   - message queue: message is received by thread
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_object_trytake_sethook(hook: unsafe extern "C" fn(*const rt_object)) {
    unsafe { rt_object_trytake_hook = Some(hook) };
}

/// This function sets a hook function, which will be invoked when an object has been taken from the kernel object system.
///
/// The object have been taken means:
///   - semaphore: semaphore have been taken by thread
///   - mutex: mutex have been taken by thread
///   - event: event have been received by thread
///   - mailbox: mail have been received by thread
///   - message queue: message have been received by thread
///   - timer: timer is started
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_object_take_sethook(hook: unsafe extern "C" fn(*const rt_object)) {
    unsafe { rt_object_take_hook = Some(hook) };
}

/// This function sets a hook function, which will be invoked when an object is put to the kernel object system.
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_object_put_sethook(hook: unsafe extern "C" fn(*const rt_object)) {
    unsafe { rt_object_put_hook = Some(hook) };
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
                print!("{:<1$}", name, $width);
            }
            Err(_) => {
                println!("Error when converting C string to UTF-8");
            }
        }
    }};
}
