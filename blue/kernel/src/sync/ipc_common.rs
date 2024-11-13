use core::ffi;
use rt_bindings;
#[macro_export]
macro_rules! rt_get_message_addr {
    ($msg:expr) => {
        ($msg as *mut rt_bindings::rt_mq_message).offset(1) as *mut _
    };
}

#[no_mangle]
pub extern "C" fn _rt_ipc_object_init(
    object: *mut rt_bindings::rt_ipc_object,
) -> rt_bindings::rt_err_t {
    unsafe {
        rt_bindings::rt_list_init!(&mut ((*object).suspend_thread));
    }

    rt_bindings::RT_EOK as rt_bindings::rt_err_t
}

#[no_mangle]
pub extern "C" fn _rt_ipc_list_resume(list: *mut rt_bindings::rt_list_t) -> rt_bindings::rt_err_t {
    unsafe {
        let thread = rt_bindings::rt_list_entry!((*list).next, rt_bindings::rt_thread, tlist)
            as *mut rt_bindings::rt_thread;
        (*thread).error = rt_bindings::RT_EOK as rt_bindings::rt_err_t;
        rt_bindings::rt_thread_resume(thread);
    }

    rt_bindings::RT_EOK as rt_bindings::rt_err_t
}

#[no_mangle]
pub extern "C" fn _rt_ipc_list_resume_all(
    list: *mut rt_bindings::rt_list_t,
) -> rt_bindings::rt_err_t {
    unsafe {
        while (*list).is_empty() == false {
            let level = rt_bindings::rt_hw_interrupt_disable();
            let thread = rt_bindings::rt_list_entry!((*list).next, rt_bindings::rt_thread, tlist)
                as *mut rt_bindings::rt_thread;
            (*thread).error = -(rt_bindings::RT_ERROR as rt_bindings::rt_err_t);
            rt_bindings::rt_thread_resume(thread);
            rt_bindings::rt_hw_interrupt_enable(level);
        }
    }

    rt_bindings::RT_EOK as rt_bindings::rt_err_t
}

#[no_mangle]
pub extern "C" fn _rt_ipc_list_suspend(
    list: *mut rt_bindings::rt_list_t,
    thread: *mut rt_bindings::rt_thread,
    flag: rt_bindings::rt_uint8_t,
    suspend_flag: i32,
) -> rt_bindings::rt_err_t {
    unsafe {
        if ((*thread).stat as u32 & rt_bindings::RT_THREAD_SUSPEND_MASK)
            != rt_bindings::RT_THREAD_SUSPEND_MASK
        {
            let ret = rt_bindings::rt_thread_suspend_with_flag(
                thread,
                suspend_flag as rt_bindings::rt_uint32_t,
            );

            if ret != rt_bindings::RT_EOK as rt_bindings::rt_err_t {
                return ret;
            }
        }

        match flag as u32 {
            rt_bindings::RT_IPC_FLAG_FIFO => {
                (*list).insert_before(&mut (*thread).tlist);
            }
            rt_bindings::RT_IPC_FLAG_PRIO => {
                let mut n = (*list).next;
                while n != list {
                    let s_thread = rt_bindings::rt_list_entry!(n, rt_bindings::rt_thread, tlist)
                        as *mut rt_bindings::rt_thread;

                    if (*thread).current_priority < (*s_thread).current_priority {
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

        rt_bindings::RT_EOK as rt_bindings::rt_err_t
    }
}
