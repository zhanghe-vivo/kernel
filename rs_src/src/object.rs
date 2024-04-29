use crate::container_of;
use crate::rt_bindings::*;
use core::ptr;

type DestroyFunc = extern "C" fn(*mut cty::c_void) -> rt_err_t;

#[derive(Debug, Copy, Clone)]
struct RtCustomObject {
    parent: rt_object,
    destroy: Option<DestroyFunc>,
    data: *mut cty::c_void,
}

/// Type to define object_info for the number of _object_container items.
enum RtObjectInfoType {
    RTObjectInfoThread = 0,
    ///< The object is a thread.
    #[cfg(feature = "RT_USING_SEMAPHORE")]
    RTObjectInfoSemaphore,
    ///< The object is a semaphore.
    #[cfg(feature = "RT_USING_MUTEX")]
    RTObjectInfoMutex,
    ///< The object is a mutex.
    #[cfg(feature = "RT_USING_EVENT")]
    RTObjectInfoEvent,
    ///< The object is an event.
    #[cfg(feature = "RT_USING_MAILBOX")]
    RTObjectInfoMailBox,
    ///< The object is a mailbox.
    #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
    RTObjectInfoMessageQueue,
    ///< The object is a message queue.
    #[cfg(feature = "RT_USING_MEMHEAP")]
    RTObjectInfoMemHeap,
    ///< The object is a memory heap.
    #[cfg(feature = "RT_USING_MEMPOOL")]
    RTObjectInfoMemPool,
    ///< The object is a memory pool.
    #[cfg(feature = "RT_USING_DEVICE")]
    RTObjectInfoDevice,
    ///< The object is a device.
    RTObjectInfoTimer,
    ///< The object is a timer.
    #[cfg(feature = "RT_USING_MODULE")]
    RTObjectInfoModule,
    ///< The object is a module.
    #[cfg(feature = "RT_USING_HEAP")]
    RTObjectInfoMemory,
    ///< The object is memory.
    #[cfg(feature = "RT_USING_SMART")]
    RTObjectInfoChannel,
    ///< The object is an IPC channel.
    #[cfg(feature = "RT_USING_HEAP")]
    RTObjectInfoCustom,
    ///< The object is a custom object.
    RTObjectInfoUnknown,
}

/// Macro to initialize object container list.
macro_rules! _obj_container_list_init {
    ($c:expr) => {
        (rt_list_t {
            next: &OBJECT_CONTAINER[$c].object_list as *const _ as *mut _,
            prev: &OBJECT_CONTAINER[$c].object_list as *const _ as *mut _,
        })
    };
}

/// Object container for different object types.
static mut OBJECT_CONTAINER: [rt_object_information;
    RtObjectInfoType::RTObjectInfoUnknown as usize] = [
    /* initialize object container - thread */
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Thread,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoThread as usize)
        },
        object_size: core::mem::size_of::<rt_thread>() as u32,
    },
    /* initialize object container - semaphore */
    #[cfg(feature = "RT_USING_SEMAPHORE")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Semaphore,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoSemaphore as usize)
        },
        object_size: core::mem::size_of::<rt_semaphore>() as u32,
    },
    /* initialize object container - mutex */
    #[cfg(feature = "RT_USING_MUTEX")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Mutex,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoMutex as usize)
        },
        object_size: core::mem::size_of::<rt_mutex>() as u32,
    },
    /* initialize object container - event */
    #[cfg(feature = "RT_USING_EVENT")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Event,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoEvent as usize)
        },
        object_size: core::mem::size_of::<rt_event>() as u32,
    },
    /* initialize object container - mailbox */
    #[cfg(feature = "RT_USING_MAILBOX")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_MailBox,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoMailBox as usize)
        },
        object_size: core::mem::size_of::<rt_mailbox>() as u32,
    },
    /* initialize object container - message queue */
    #[cfg(feature = "RT_USING_MESSAGEQUEUE")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_MessageQueue,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoMessageQueue as usize)
        },
        object_size: core::mem::size_of::<rt_messagequeue>() as u32,
    },
    /* initialize object container - memory heap */
    #[cfg(feature = "RT_USING_MEMHEAP")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_MemHeap,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoMemHeap as usize)
        },
        object_size: core::mem::size_of::<rt_memheap>() as u32,
    },
    /* initialize object container - memory pool */
    #[cfg(feature = "RT_USING_MEMPOOL")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_MemPool,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoMemPool as usize)
        },
        object_size: core::mem::size_of::<rt_mempool>() as u32,
    },
    /* initialize object container - device */
    #[cfg(feature = "RT_USING_DEVICE")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Device,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoDevice as usize)
        },
        object_size: core::mem::size_of::<rt_device>() as u32,
    },
    /* initialize object container - timer */
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Timer,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoTimer as usize)
        },
        object_size: core::mem::size_of::<rt_timer>() as u32,
    },
    /* initialize object container - module */
    #[cfg(feature = "RT_USING_MODULE")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Module,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoModule as usize)
        },
        object_size: core::mem::size_of::<rt_dlmodule>() as u32,
    },
    /* initialize object container - event */
    #[cfg(feature = "RT_USING_HEAP")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Memory,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoMemory as usize)
        },
        object_size: core::mem::size_of::<rt_memory>() as u32,
    },
    /* initialize object container - event */
    #[cfg(feature = "RT_USING_SMART")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Channel,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoChannel as usize)
        },
        object_size: core::mem::size_of::<rt_channel>() as u32,
    },
    /* initialize object container - event */
    #[cfg(feature = "RT_USING_HEAP")]
    rt_object_information {
        type_: rt_object_class_type_RT_Object_Class_Custom,
        object_list: unsafe {
            _obj_container_list_init!(RtObjectInfoType::RTObjectInfoCustom as usize)
        },
        object_size: core::mem::size_of::<RtCustomObject>() as u32,
    },
];

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_object_attach_hook: Option<unsafe extern "C" fn(*const rt_object)> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_object_detach_hook: Option<unsafe extern "C" fn(*const rt_object)> = None;
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
    unsafe { rt_object_attach_hook = Some(hook) };
}

/// This function sets a hook function, which will be invoked when an object detaches from the kernel object system.
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_object_detach_sethook(hook: unsafe extern "C" fn(*const rt_object)) {
    unsafe { rt_object_detach_hook = Some(hook) };
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
) -> *mut rt_object_information {
    for index in 0..RtObjectInfoType::RTObjectInfoUnknown as usize {
        if unsafe { OBJECT_CONTAINER[index].type_ } == object_type {
            return unsafe { &OBJECT_CONTAINER[index] } as *const _ as *mut _;
        }
    }
    core::ptr::null_mut()
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
    let mut count = 0;
    let mut node: *const rt_list_node = core::ptr::null();
    let information = rt_object_get_information(object_type);

    if !information.is_null() {
        unsafe {
            let level = rt_hw_interrupt_disable();
            crate::rt_list_for_each!(
                node,
                &((*information).object_list) as *const rt_list_node as *mut rt_list_node,
                {
                    count += 1;
                }
            );

            rt_hw_interrupt_enable(level);
        }
    }
    count
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
pub extern "C" fn rt_object_get_pointers(
    object_type: rt_object_class_type,
    pointers: *mut rt_object_t,
    maxlen: usize,
) -> usize {
    if maxlen == 0 {
        return 0;
    }

    let mut index = 0usize;

    let mut node: *const rt_list_node = core::ptr::null();
    let information = rt_object_get_information(object_type);

    if !information.is_null() {
        unsafe {
            let level = rt_hw_interrupt_disable();

            crate::rt_list_for_each!(
                node,
                &((*information).object_list) as *const rt_list_node as *mut rt_list_node,
                {
                    let object = crate::rt_list_entry!(node, rt_object, list);
                    let offset_pointer = unsafe { pointers.add(index) };
                    *offset_pointer = object as *mut _;
                    index += 1;
                    if index >= maxlen {
                        break;
                    }
                }
            );
            rt_hw_interrupt_enable(level);
        }
    }
    index
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
    name: *const u8,
) {
    let information = rt_object_get_information(type_);
    assert!(!information.is_null());

    #[cfg(feature = "RT_USING_DEBUG")]
    unsafe {
        let mut node = (*information).object_list.next;
        rt_enter_critical();
        loop {
            if ptr::eq(node, &(*information).object_list) {
                break;
            }
            let obj = crate::rt_list_entry!(node, rt_object, list);
            assert!(!ptr::eq(obj, object));
            node = (*node).next;
        }
        rt_exit_critical();
    }

    // initialize object's parameters
    // set object type to static
    let object_type = type_ as u8 | rt_object_class_type_RT_Object_Class_Static as u8;
    unsafe {
        (*object).type_ = object_type;
    }

    #[cfg(feature = "RT_NAME_MAX")]
    unsafe {
        rt_strncpy((*object).name.as_mut_ptr(), name, RT_NAME_MAX);
    }
    #[cfg(not(feature = "RT_NAME_MAX"))]
    unsafe {
        (*object).name = name;
    }

    unsafe {
        crate::rt_object_hook_call!(rt_object_attach_hook, object);
    }

    #[cfg(feature = "RT_USING_MODULE")]
    let module = unsafe { dlmodule_self() };

    let level = unsafe { rt_hw_interrupt_disable() };

    #[cfg(feature = "RT_USING_MODULE")]
    if !module.is_null() {
        unsafe {
            (*module)
                .object_list
                .insert_after(&(*object).list as *const _ as *mut _);
            (*object).module_id = module as *mut cty::c_void;
        }
    } else {
        // insert object into information object list
        unsafe {
            (*information)
                .object_list
                .insert_after(&(*object).list as *const _ as *mut _);
        }
    }

    #[cfg(not(feature = "RT_USING_MODULE"))]
    unsafe {
        (*information)
            .object_list
            .insert_after(&(*object).list as *const _ as *mut _);
    }

    unsafe { rt_hw_interrupt_enable(level) };
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

    unsafe {
        crate::rt_object_hook_call!(rt_object_detach_hook, object);
        (*object).type_ = 0;
        let level = rt_hw_interrupt_disable();
        (*object).list.remove();
        rt_hw_interrupt_enable(level);
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
pub extern "C" fn rt_object_allocate(type_: rt_object_class_type, name: *const u8) -> rt_object_t {
    // get object information
    let information = rt_object_get_information(type_);
    assert!(!information.is_null());

    crate::rt_debug_not_in_interrupt!();

    let object = unsafe { rt_malloc((*information).object_size) as *mut rt_object };
    if object.is_null() {
        return object;
    }

    unsafe {
        rt_memset(object as *mut cty::c_void, 0x0, (*information).object_size);
        (*object).type_ = type_ as u8;
        (*object).flag = 0;
        #[cfg(feature = "RT_NAME_MAX")]
        rt_strncpy((*object).name.as_mut_ptr(), name, RT_NAME_MAX);
    }

    #[cfg(not(feature = "RT_NAME_MAX"))]
    unsafe {
        (*object).name = name;
    }

    unsafe {
        crate::rt_object_hook_call!(rt_object_attach_hook, object);
    }

    #[cfg(feature = "RT_USING_MODULE")]
    let module = unsafe { dlmodule_self() };

    let level = unsafe { rt_hw_interrupt_disable() };

    #[cfg(feature = "RT_USING_MODULE")]
    if !module.is_null() {
        unsafe {
            (*module)
                .object_list
                .insert_after(&(*object).list as *const _ as *mut _);
            (*object).module_id = module as *mut cty::c_void;
        }
    } else {
        // insert object into information object list
        unsafe {
            (*information)
                .object_list
                .insert_after(&(*object).list as *const _ as *mut _);
        }
    }

    #[cfg(not(feature = "RT_USING_MODULE"))]
    unsafe {
        (*information)
            .object_list
            .insert_after(&(*object).list as *const _ as *mut _);
    }

    unsafe {
        rt_hw_interrupt_enable(level);
    }

    object
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
        assert!(((*object).type_ & rt_object_class_type_RT_Object_Class_Static as u8) == 0);
    }

    unsafe {
        crate::rt_object_hook_call!(rt_object_detach_hook, object);
        // reset object type
        (*object).type_ = rt_object_class_type_RT_Object_Class_Null as u8;
        // lock interrupt
        let level = rt_hw_interrupt_disable();
        // remove from old list
        (*object).list.remove();
        rt_hw_interrupt_enable(level);
        rt_free(object as *mut cty::c_void);
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
    name: *const cty::c_char,
    data: *mut cty::c_void,
    data_destroy: DestroyFunc,
) -> rt_object_t {
    let cobj = rt_object_allocate(rt_object_class_type_RT_Object_Class_Custom, name)
        as *mut RtCustomObject;
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
    let cobj = obj as *mut RtCustomObject;

    if !obj.is_null()
        && (unsafe { *obj }).type_ == rt_object_class_type_RT_Object_Class_Custom as u8
    {
        if let Some(destroy_fn) = unsafe { (*cobj).destroy } {
            ret = destroy_fn((unsafe { *cobj }).data);
        }
        rt_object_delete(obj);
    }
    ret
}

/// This function will judge the object is system object or not.
///
/// Normally, the system object is a static object and the type
/// of object set to rt_object_class_type_RT_Object_Class_Static.
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

    if (obj_type & rt_object_class_type_RT_Object_Class_Static as u8) != 0 {
        return RT_TRUE as cty::c_int;
    }

    return RT_FALSE as cty::c_int;
}

/// This function will return the type of object without
/// `rt_object_class_type_RT_Object_Class_Static` flag.
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

    return obj_type & (!rt_object_class_type_RT_Object_Class_Static as u8);
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
pub extern "C" fn rt_object_find(name: *const cty::c_char, type_: rt_uint8_t) -> rt_object_t {
    let information = rt_object_get_information(type_ as u32);

    /* parameter check */
    if name.is_null() || information.is_null() {
        return ptr::null_mut();
    }

    /* which is invoke in interrupt status */
    crate::rt_debug_not_in_interrupt!();

    let mut object: rt_object_t = ptr::null_mut();
    let mut node: *mut rt_list_node = ptr::null_mut();
    unsafe {
        /* enter critical */
        rt_enter_critical();

        /* try to find object */
        crate::rt_list_for_each!(
            node,
            &(*information).object_list as *const rt_list_node as *mut rt_list_node,
            {
                object = crate::rt_list_entry!(node, rt_object, list) as *mut rt_object;
                if rt_strncmp(
                    (*object).name.as_ptr() as *const cty::c_char,
                    name,
                    RT_NAME_MAX,
                ) == 0
                {
                    /* leave critical */
                    unsafe { rt_exit_critical() };
                    return object;
                }
            }
        );

        /* leave critical */
        rt_exit_critical();
    }

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
    name: *const cty::c_char,
    name_size: rt_uint8_t,
) -> rt_err_t {
    let mut result: rt_err_t = -(RT_EINVAL as i32);
    if !object.is_null() && !name.is_null() && name_size != 0 {
        let obj_name = (unsafe { *object }).name;
        unsafe { rt_strncpy(name as *mut _, obj_name.as_ptr(), name_size as rt_size_t) };
        result = RT_EOK as rt_err_t;
    }

    return result;
}
