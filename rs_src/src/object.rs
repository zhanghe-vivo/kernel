use crate::{
    allocator::{rt_free, rt_malloc},
    klibc::{rt_memset, rt_strncmp, rt_strncpy},
    linked_list::ListHead,
    rt_bindings::*,
    scheduler::{rt_enter_critical, rt_exit_critical},
    static_init::UnsafeStaticInit,
    sync::RawSpin,
    thread::RtThread,
};

use core::{ffi, mem, pin::Pin, ptr, slice};
use pinned_init::*;

type DestroyFunc = extern "C" fn(*mut ffi::c_void) -> rt_err_t;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectClassType {
    ObjectClassUninit = 0,
    ObjectClassThread = 1,
    //< The object is a thread.
    #[cfg(feature = "RT_USING_SEMAPHORE")]
    ObjectClassSemaphore,
    //< The object is a semaphore.
    #[cfg(feature = "RT_USING_MUTEX")]
    ObjectClassMutex,
    //< The object is a mutex.
    #[cfg(feature = "RT_USING_EVENT")]
    ObjectClassEvent,
    //< The object is an event.
    #[cfg(feature = "RT_USING_MAILBOX")]
    ObjectClassMailBox,
    //< The object is a mailbox.
    #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
    ObjectClassMessageQueue,
    //< The object is a message queue.
    #[cfg(feature = "RT_USING_MEMHEAP")]
    ObjectClassMemHeap,
    //< The object is a memory heap.
    #[cfg(feature = "RT_USING_MEMPOOL")]
    ObjectClassMemPool,
    //< The object is a memory pool.
    #[cfg(feature = "RT_USING_DEVICE")]
    ObjectClassDevice,
    //< The object is a device.
    ObjectClassTimer,
    //< The object is a timer.
    #[cfg(feature = "RT_USING_MODULE")]
    ObjectClassModule,
    //< The object is a module.
    #[cfg(feature = "RT_USING_HEAP")]
    ObjectClassMemory,
    //< The object is memory.
    #[cfg(feature = "RT_USING_SMART")]
    ObjectClassChannel,
    //< The object is an IPC channel.
    #[cfg(feature = "RT_USING_HEAP")]
    ObjectClassCustom,
    //< The object is a custom object.
    ObjectClassUnknown,
}

/// The object is a static object.
const OBJECT_CLASS_STATIC: u8 = 0x80;

#[pin_data]
//#[derive(Debug)]
#[repr(C)]
pub struct ObjectInformation {
    pub(crate) spinlock: RawSpin,
    #[pin]
    pub(crate) object_list: ListHead,
    object_size: usize,
    obj_type: ObjectClassType,
}

#[doc = " Base structure of Kernel object"]
#[pin_data]
#[derive(Debug)]
#[repr(C)]
pub struct BaseObject {
    #[doc = "< dynamic name of kernel object"]
    pub name: [i8; RT_NAME_MAX as usize],
    #[doc = "< type of kernel object"]
    pub type_: u8,
    #[doc = "< flag of kernel object"]
    pub flag: u8,
    #[doc = "< list node of kernel object"]
    #[pin]
    list: ListHead,
}

#[pin_data]
#[derive(Debug)]
#[repr(C)]
struct CustomObject {
    #[pin]
    parent: BaseObject,
    destroy: Option<DestroyFunc>,
    data: *mut ffi::c_void,
}

#[pin_data]
pub(crate) struct ObjectContainer {
    #[pin]
    data: [ObjectInformation; ObjectClassType::ObjectClassUnknown as usize - 1],
}

pub(crate) static mut OBJECT_CONTAINER: UnsafeStaticInit<ObjectContainer, ObjectContainerInit> =
    UnsafeStaticInit::new(ObjectContainerInit);

pub(crate) struct ObjectContainerInit;
unsafe impl PinInit<ObjectContainer> for ObjectContainerInit {
    unsafe fn __pinned_init(
        self,
        slot: *mut ObjectContainer,
    ) -> Result<(), core::convert::Infallible> {
        let init = ObjectContainer::new();
        unsafe { init.__pinned_init(slot) }
    }
}

impl ObjectContainer {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            data <- pin_init_array_from_fn(|i| ObjectInformation::new(i as u8)),
        })
    }
}

impl ObjectClassType {
    // 为枚举类型添加方法
    fn get_object_size(index: u8) -> usize {
        match index {
            x if x == Self::ObjectClassThread as u8 => mem::size_of::<RtThread>(),
            //< The object is a thread.
            #[cfg(feature = "RT_USING_SEMAPHORE")]
            x if x == Self::ObjectClassSemaphore as u8 => mem::size_of::<rt_semaphore>(),
            //< The object is a semaphore.
            #[cfg(feature = "RT_USING_MUTEX")]
            x if x == Self::ObjectClassMutex as u8 => mem::size_of::<rt_mutex>(),
            //< The object is a mutex.
            #[cfg(feature = "RT_USING_EVENT")]
            x if x == Self::ObjectClassEvent as u8 => mem::size_of::<rt_event>(),
            //< The object is an event.
            #[cfg(feature = "RT_USING_MAILBOX")]
            x if x == Self::ObjectClassMailBox as u8 => mem::size_of::<rt_mailbox>(),
            //< The object is a mailbox.
            #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
            x if x == Self::ObjectClassMessageQueue as u8 => mem::size_of::<rt_messagequeue>(),
            //< The object is a message queue.
            #[cfg(feature = "RT_USING_MEMHEAP")]
            x if x == Self::ObjectClassMemHeap as u8 => mem::size_of::<rt_memheap>(),
            //< The object is a memory heap.
            #[cfg(feature = "RT_USING_MEMPOOL")]
            x if x == Self::ObjectClassMemPool as u8 => mem::size_of::<rt_mempool>(),
            //< The object is a memory pool.
            #[cfg(feature = "RT_USING_DEVICE")]
            x if x == Self::ObjectClassDevice as u8 => mem::size_of::<rt_device>(),
            //< The object is a device.
            x if x == Self::ObjectClassTimer as u8 => mem::size_of::<rt_timer>(),
            //< The object is a timer.
            #[cfg(feature = "RT_USING_MODULE")]
            x if x == Self::ObjectClassModule as u8 => mem::size_of::<rt_dlmodule>(),
            //< The object is a module.
            #[cfg(feature = "RT_USING_HEAP")]
            x if x == Self::ObjectClassMemory as u8 => mem::size_of::<rt_memory>(),
            //< The object is memory.
            #[cfg(feature = "RT_USING_SMART")]
            x if x == Self::ObjectClassChannel as u8 => mem::size_of::<rt_channel>(),
            //< The object is an IPC channel.
            #[cfg(feature = "RT_USING_HEAP")]
            x if x == Self::ObjectClassCustom as u8 => mem::size_of::<CustomObject>(),

            _ => unreachable!("not a kernel object type!"),
        }
    }
}

impl ObjectInformation {
    fn new(index: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            spinlock: RawSpin::new(),
            object_list <- ListHead::new(),
            object_size: ObjectClassType::get_object_size(index + 1),
            obj_type: unsafe { mem::transmute(index + 1) },
        })
    }

    #[inline]
    pub fn get_info_by_type(object_type: u8) -> Option<&'static ObjectInformation> {
        if object_type > ObjectClassType::ObjectClassUninit as u8
            && object_type < ObjectClassType::ObjectClassUnknown as u8
        {
            Some(unsafe { &OBJECT_CONTAINER.data[(object_type - 1) as usize] })
        } else {
            None
        }
    }

    pub fn size(object_type: u8) -> usize {
        if object_type > ObjectClassType::ObjectClassUninit as u8
            && object_type < ObjectClassType::ObjectClassUnknown as u8
        {
            let info = unsafe { &OBJECT_CONTAINER.data[(object_type - 1) as usize] };
            info.spinlock.acquire();
            info.object_list.size()
        } else {
            0
        }
    }

    pub fn get_objects_by_type(object_type: u8, objects: &mut [*mut BaseObject]) -> usize {
        if object_type > ObjectClassType::ObjectClassUninit as u8
            && object_type < ObjectClassType::ObjectClassUnknown as u8
        {
            let mut count: usize = 0;
            let maxlen: usize = objects.len();
            let info = unsafe { &OBJECT_CONTAINER.data[(object_type - 1) as usize] };
            info.spinlock.acquire();
            crate::list_head_for_each!(node, &info.object_list, {
                let object = unsafe { crate::list_head_entry!(node.as_ptr(), BaseObject, list) };
                objects[count] = object as *mut BaseObject;
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
}

/// This function will return the specified type of object information.
///
/// # Arguments
///
/// * `object_type` - The type of object, which can be RT_Object_Class_Thread, Semaphore, Mutex, etc.
///
/// # Returns
///
/// The object type information or None if not found.
#[no_mangle]
pub extern "C" fn rt_object_get_information(
    object_type: rt_object_class_type,
) -> *const rt_object_information {
    if let Some(info) =
        ObjectInformation::get_info_by_type(object_type as u8 & (!OBJECT_CLASS_STATIC))
    {
        info as *const _ as *const rt_object_information
    } else {
        core::ptr::null_mut()
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
    ObjectInformation::size(object_type as u8)
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

    let object_slice: &mut [*mut BaseObject] =
        slice::from_raw_parts_mut(pointers as *mut *mut BaseObject, maxlen);
    ObjectInformation::get_objects_by_type(object_type as u8, object_slice)
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
    let information = ObjectInformation::get_info_by_type(type_ as u8).unwrap();

    #[cfg(feature = "RT_USING_DEBUG")]
    {
        let _guard = information.spinlock.acquire();
        crate::list_head_for_each!(node, &information.object_list, {
            let obj = unsafe { crate::list_head_entry!(node.as_ptr(), BaseObject, list) };
            assert!(!ptr::eq(object, obj as *const rt_object));
        });
    }

    let obj_ref = unsafe { &mut *(object as *mut BaseObject) };
    // initialize object's parameters
    // set object type to static
    let type_ = type_ as u8 | OBJECT_CLASS_STATIC;

    rt_object_init_internal(information, obj_ref, type_, name);
}

pub(crate) fn rt_object_init_dyn(
    object: *mut rt_object,
    type_: rt_object_class_type,
    name: *const ffi::c_char,
) {
    assert!(!object.is_null());

    let information = ObjectInformation::get_info_by_type(type_ as u8).unwrap();
    let obj_ref = unsafe { &mut *(object as *mut BaseObject) };

    rt_object_init_internal(information, obj_ref, type_ as u8, name);
}

#[inline]
fn rt_object_init_internal(
    information: &ObjectInformation,
    obj_ref: &mut BaseObject,
    type_: u8,
    name: *const ffi::c_char,
) {
    obj_ref.type_ = type_;

    #[cfg(feature = "RT_NAME_MAX")]
    unsafe {
        rt_strncpy(obj_ref.name.as_mut_ptr(), name, (RT_NAME_MAX - 1) as usize);
    }

    #[cfg(not(feature = "RT_NAME_MAX"))]
    {
        obj_ref.name = name;
    }

    unsafe {
        crate::rt_object_hook_call!(OBJECT_ATTACH_HOOK, obj_ref as *const _ as *const rt_object);
    }

    #[cfg(feature = "RT_USING_MODULE")]
    let module = unsafe { dlmodule_self() };
    // let _ = unsafe { ListHead::new().__pinned_init(&mut obj_ref.list as *mut ListHead) };
    let _guard = information.spinlock.acquire();
    #[cfg(feature = "RT_USING_MODULE")]
    if !module.is_null() {
        unsafe {
            Pin::new_unchecked(&mut obj_ref.list).insert_next(&(*module).object_list);
            obj_ref.module_id = module as *mut ffi::c_void;
        }
    } else {
        // insert object into information object list
        unsafe {
            Pin::new_unchecked(&mut obj_ref.list).insert_next(&information.object_list);
        }
    }

    #[cfg(not(feature = "RT_USING_MODULE"))]
    unsafe {
        Pin::new_unchecked(&mut obj_ref.list).insert_next(&information.object_list);
    }
    #[cfg(feature = "RT_USING_DEBUG")]
    {
        assert!(ptr::eq(
            &obj_ref.list,
            information.object_list.next.as_ptr()
        ));
        assert!(ptr::eq(
            obj_ref.list.prev.as_ptr(),
            &information.object_list
        ));
        let mut count: u32 = 0;
        crate::list_head_for_each!(node, &information.object_list, {
            if count > 1 {
                assert!(!ptr::eq(node.next.as_ptr(), node.prev.as_ptr()));
            }
            count += 1;
            assert!(count < 100);
        });
    }
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
    unsafe { crate::rt_object_hook_call!(OBJECT_DETACH_HOOK, object) };

    //let obj = unsafe { Box::leak(Box::from_raw(object as *mut BaseObject)) };
    let obj = unsafe { &mut *(object as *mut BaseObject) };
    if let Some(information) =
        ObjectInformation::get_info_by_type(obj.type_ & (!OBJECT_CLASS_STATIC))
    {
        information.spinlock.acquire();
        unsafe { Pin::new_unchecked(&mut obj.list).remove() };
        obj.type_ = ObjectClassType::ObjectClassUninit as u8;
    } else {
        panic!(
            "object type not find. name: {:?}, type: {:?}",
            obj.name, obj.type_
        );
    }
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
    // get object information
    use core::ffi::c_void;
    let information = ObjectInformation::get_info_by_type(type_ as u8).unwrap();

    crate::rt_debug_not_in_interrupt!();

    let object = unsafe { rt_malloc(information.object_size) as *mut BaseObject };
    if object.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        rt_memset(object as *mut c_void, 0x0, information.object_size);
    }

    let obj_ref = unsafe { &mut *object };
    rt_object_init_internal(information, obj_ref, type_ as u8, name);
    object as rt_object_t
}

/// This function will delete an object and release object memory.
///
/// object is the specified object to be deleted.
#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_object_delete(object: rt_object_t) {
    // object check
    assert!(!object.is_null());
    unsafe {
        assert!(((*object).type_ & OBJECT_CLASS_STATIC) == 0);
    }

    unsafe { crate::rt_object_hook_call!(OBJECT_DETACH_HOOK, object) };

    unsafe {
        let obj = &mut *object;
        let information = ObjectInformation::get_info_by_type(obj.type_).unwrap();
        // lock interrupt
        {
            let _guard = information.spinlock.acquire();
            Pin::new_unchecked(&mut obj.list).remove();
        }
        // reset object type
        obj.type_ = ObjectClassType::ObjectClassUninit as u8;
        rt_free(object as *mut ffi::c_void);
    }
}

/// This function will create a custom object container.
///
/// # Arguments
///
/// * `name` - the specified name of object.
/// * `data` - the custom data.
/// * `data_destroy` - the custom object destroy callback.
///
/// # Returns
///
/// The found object or `RT_NULL` if there is no this object in object container.
///
/// # Note
///
/// This function shall not be invoked in interrupt status.
#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_custom_object_create(
    name: *const ffi::c_char,
    data: *mut ffi::c_void,
    data_destroy: DestroyFunc,
) -> rt_object_t {
    let cobj =
        rt_object_allocate(ObjectClassType::ObjectClassCustom as u32, name) as *mut CustomObject;
    if cobj.is_null() {
        return cobj as rt_object_t;
    }
    unsafe {
        (*cobj).destroy = Some(data_destroy);
        (*cobj).data = data;
    }
    cobj as rt_object_t
}

/// This function will destroy a custom object container.
///
/// # Arguments
///
/// * `obj` - the specified name of object.
///
/// # Note
///
/// This function shall not be invoked in interrupt status.
#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_custom_object_destroy(obj: *mut rt_object) -> rt_err_t {
    let mut ret: rt_err_t = -1;
    let cobj = obj as *mut CustomObject;

    if !obj.is_null() && (unsafe { *obj }).type_ == ObjectClassType::ObjectClassCustom as u8 {
        let custom_obj = unsafe { &*cobj };
        if let Some(destroy_fn) = custom_obj.destroy {
            ret = destroy_fn(custom_obj.data);
        }
        rt_object_delete(obj);
    }
    ret
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
    let obj_type = unsafe { (*object).type_ };

    if (obj_type & OBJECT_CLASS_STATIC) != 0 {
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
    let obj_type = unsafe { (*object).type_ };

    return obj_type & (!OBJECT_CLASS_STATIC);
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
    /* parameter check */
    if name.is_null() {
        return ptr::null_mut();
    }

    /* which is invoke in interrupt status */
    crate::rt_debug_not_in_interrupt!();

    let information = ObjectInformation::get_info_by_type(type_ & (!OBJECT_CLASS_STATIC)).unwrap();
    /* enter critical */
    rt_enter_critical();
    /* try to find object */
    crate::list_head_for_each!(node, &information.object_list, {
        unsafe {
            let object = crate::list_head_entry!(node.as_ptr(), BaseObject, list);
            if rt_strncmp(
                (*object).name.as_ptr() as *const ffi::c_char,
                name,
                RT_NAME_MAX as usize,
            ) == 0
            {
                /* leave critical */
                rt_exit_critical();
                return object as rt_object_t;
            }
        }
    });

    /* leave critical */
    rt_exit_critical();

    ptr::null_mut()
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

