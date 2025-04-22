use crate::kernel::{
    error::code,
    object::{KObjectBase, KernelObject, ObjectClassType},
    process,
};
use bluekernel_infra::string;
use core::{ffi, slice};

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
pub extern "C" fn rt_object_get_length(object_type: u8) -> usize {
    process::get_object_size(ObjectClassType::from(object_type))
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
    object_type: u8,
    pointers: *mut KObjectBase,
    maxlen: usize,
) -> usize {
    if maxlen == 0 {
        return 0;
    }

    let object_slice: &mut [*mut KObjectBase] =
        slice::from_raw_parts_mut(pointers as *mut *mut KObjectBase, maxlen);
    process::get_objects_by_type(ObjectClassType::from(object_type), object_slice)
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
pub extern "C" fn rt_object_init(object: *mut KObjectBase, type_: u8, name: *const ffi::c_char) {
    assert!(!object.is_null());
    let obj_ref = unsafe { &mut *(object as *mut KObjectBase) };
    #[cfg(feature = "debugging_object")]
    process::object_addr_detect(ObjectClassType::from(type_), obj_ref);
    // initialize object's parameters
    // set object type to static
    obj_ref.init(ObjectClassType::from(type_), name);
}

/// This function will detach a static object from the object system,
/// and the memory of the static object is not freed.
///
/// # Arguments
///
/// * `object` - The specified object to be detached.
#[no_mangle]
pub extern "C" fn rt_object_detach(object: *mut KObjectBase) {
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
#[cfg(feature = "heap")]
#[no_mangle]
pub extern "C" fn rt_object_allocate(type_: u8, name: *const ffi::c_char) -> *mut KObjectBase {
    KObjectBase::new_raw(ObjectClassType::from(type_), name)
}

/// This function will delete an object and release object memory.
///
/// object is the specified object to be deleted.
#[cfg(feature = "heap")]
#[no_mangle]
pub extern "C" fn rt_object_delete(object: *mut KObjectBase) {
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
pub extern "C" fn rt_object_is_systemobject(object: *mut KObjectBase) -> i32 {
    /* object check */
    assert!(!object.is_null());
    let obj = unsafe { &mut *(object as *mut KObjectBase) };
    let res = obj.is_static_kobject();

    if res {
        return code::TRUE.to_errno();
    }

    return code::FLASE.to_errno();
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
pub extern "C" fn rt_object_get_type(object: *mut KObjectBase) -> u8 {
    /* object check */
    assert!(!object.is_null());
    let obj = unsafe { &mut *(object as *mut KObjectBase) };
    obj.type_name() as u8
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
pub extern "C" fn rt_object_find(name: *const ffi::c_char, type_: u8) -> *mut KObjectBase {
    process::find_object(ObjectClassType::from(type_), name) as *mut KObjectBase
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
    object: *mut KObjectBase,
    name: *const ffi::c_char,
    name_size: u8,
) -> i32 {
    let mut result: i32 = code::EINVAL.to_errno();
    if !object.is_null() && !name.is_null() && name_size != 0 {
        let obj_name = (unsafe { &mut *object }).name;
        unsafe { string::strncpy(name as *mut _, obj_name.as_ptr(), name_size as usize) };
        result = code::EOK.to_errno();
    }

    return result;
}

#[no_mangle]
pub extern "C" fn rt_object_for_each_callback(
    obj_type: u8,
    callback_fn: extern "C" fn(*mut KObjectBase, usize, *mut ffi::c_void),
    args: *mut ffi::c_void,
) {
    let _ = process::bindings_foreach(callback_fn, ObjectClassType::from(obj_type), args);
}
