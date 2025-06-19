use core::ptr;
use libc::{c_char, c_int, c_uint, sem_t};

pub type RsSemaphore = crate::sync::semaphore::Semaphore;

#[no_mangle]
pub unsafe extern "C" fn sem_close(sem: *mut sem_t) -> c_int {
    ptr::drop_in_place(sem.cast::<RsSemaphore>());
    0
}

#[no_mangle]
pub unsafe extern "C" fn sem_destroy(sem: *mut sem_t) -> c_int {
    ptr::drop_in_place(sem.cast::<RsSemaphore>());
    0
}

#[no_mangle]
pub unsafe extern "C" fn sem_getvalue(sem: *mut sem_t, sval: *mut c_int) -> c_int {
    sval.write(get(sem).value() as c_int);

    0
}

#[no_mangle]
pub unsafe extern "C" fn sem_init(sem: *mut sem_t, _pshared: c_int, value: c_uint) -> c_int {
    sem.cast::<RsSemaphore>()
        .write(RsSemaphore::new(value as usize));

    0
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_open.html>.
// TODO: va_list
// #[no_mangle]
pub unsafe extern "C" fn sem_open(
    _name: *const c_char,
    _oflag: c_int, /* (va_list) value: c_uint */
) -> *mut sem_t {
    todo!("named semaphores")
}

#[no_mangle]
pub unsafe extern "C" fn sem_post(sem: *mut sem_t) -> c_int {
    get(sem).post(1);

    0
}

#[no_mangle]
pub unsafe extern "C" fn sem_trywait(sem: *mut sem_t) -> c_int {
    get(sem).try_wait();
    0
}

#[no_mangle]
pub unsafe extern "C" fn sem_unlink(_name: *const c_char) -> c_int {
    todo!("named semaphores")
}

#[allow(unused)]
#[no_mangle]
pub unsafe extern "C" fn sem_wait(sem: *mut sem_t) -> c_int {
    get(sem).wait(None);

    0
}

unsafe fn get<'any>(sem: *mut sem_t) -> &'any RsSemaphore {
    &*sem.cast()
}
