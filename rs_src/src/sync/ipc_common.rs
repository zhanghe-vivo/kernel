use core::{ffi, ptr};
use crate::rt_bindings::*;
use crate::{rt_list_init, rt_list_entry, container_of};
#[macro_export]
macro_rules! rt_get_message_addr {
    ($msg:expr) => {
        ($msg as *mut rt_mq_message).offset(1) as *mut _
    };
}

#[no_mangle]
pub extern "C" fn _rt_ipc_object_init(object: *mut rt_ipc_object) -> rt_err_t{
    unsafe {
        rt_list_init!(&mut ((*object).suspend_thread));
    }

    RT_EOK as rt_err_t
}

#[no_mangle]
pub extern "C" fn _rt_ipc_list_resume(list: *mut rt_list_t) -> rt_err_t{

    unsafe {
        let thread = rt_list_entry!((*list).next, rt_thread, tlist) as *mut rt_thread;
        (*thread).error = RT_EOK as rt_err_t;
        rt_thread_resume(thread);
    }

    RT_EOK as rt_err_t
}

#[no_mangle]
pub extern "C" fn _rt_ipc_list_resume_all(list: *mut rt_list_t) -> rt_err_t{
    unsafe
    {
        while (*list).is_empty() == false {
            let level = rt_hw_interrupt_disable();
            let thread = rt_list_entry!((*list).next, rt_thread, tlist) as *mut rt_thread;
            (*thread).error = -(RT_ERROR as rt_err_t);
            rt_thread_resume(thread);
            rt_hw_interrupt_enable(level);
        }
    }

    RT_EOK as rt_err_t
}

#[no_mangle]
pub extern "C" fn _rt_ipc_list_suspend(list : *mut  rt_list_t, thread : * mut rt_thread, flag : rt_uint8_t, suspend_flag : i32) -> rt_err_t
{
unsafe {
    if ((*thread).stat as u32 & RT_THREAD_SUSPEND_MASK) != RT_THREAD_SUSPEND_MASK {
        let ret = rt_thread_suspend_with_flag(thread, suspend_flag);

        if ret != RT_EOK as rt_err_t {
            return ret;
        }
    }

    match flag as u32{
        RT_IPC_FLAG_FIFO => {
            (*list).insert_before(&mut (*thread).tlist);
        }
        RT_IPC_FLAG_PRIO => {
            let mut n = (*list).next;
            while n != list
            {
                let s_thread = rt_list_entry!(n, rt_thread, tlist) as *mut rt_thread;

                if ((*thread).current_priority < (*s_thread).current_priority)
                {
                    let insert_to = &mut ((*s_thread).tlist);
                    insert_to.insert_before(&mut ((*thread).tlist));
                }
                n = (*n).next;
            }

            if n == list {
                (*list).insert_before(&mut (*thread).tlist);
            }
        }
        _ => {
            assert!(false);
        }
    }

    RT_EOK as rt_err_t

    }
}

#[no_mangle]
pub unsafe extern "C" fn _rt_memcpy(dst: *mut ffi::c_void, src: * const ffi::c_void, size: usize) -> *mut ffi::c_void {
    dst.copy_from(src, size);
    dst
}
