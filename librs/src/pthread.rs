use crate::{
    free, posix_memalign,
    sync::{
        cond::{Cond, CondAttr},
        mutex::{Mutex, MutexAttr},
        waitval::Waitval,
    },
};
use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use bluekernel_header::{
    syscalls::NR::{CreateThread, ExitThread, GetTid},
    thread::{CloneArgs, DEFAULT_STACK_SIZE, STACK_ALIGN},
};
use bluekernel_scal::bk_syscall;
use core::{
    ffi::{c_int, c_size_t, c_void},
    intrinsics::transmute,
    sync::atomic::{AtomicI8, AtomicUsize, Ordering},
};
use libc::{
    clockid_t, pthread_attr_t, pthread_cond_t, pthread_condattr_t, pthread_key_t, pthread_mutex_t,
    pthread_mutexattr_t, pthread_t, EDEADLK, EINVAL, ESRCH,
};
use spin::RwLock;

pub type PosixRoutineEntry = extern "C" fn(arg: *mut c_void) -> *mut c_void;

#[repr(C)]
struct InnerPthreadAttr {
    pub stack_size: usize,
    padding: [usize; 4],
}

// TODO: Current BlueOS kernel doesn't feature using thread-pointer pointing to TCB. Use a global map temporarily.
pub static TCBS: RwLock<BTreeMap<pthread_t, Arc<RwLock<PthreadTcb>>>> =
    RwLock::new(BTreeMap::new());
// TODO: Maybe store KEYS in BlueProcess.
static KEYS: RwLock<BTreeMap<pthread_key_t, Dtor>> = RwLock::new(BTreeMap::new());
struct Dtor(Option<extern "C" fn(value: *mut c_void)>);
static KEY_COUNTER: AtomicUsize = AtomicUsize::new(0);

// We are not exposing kernel thread to user level libc, maintain pthread's tcb by libc itself.
struct PthreadTcb {
    // FIXME: Rust doesn't allow *mut T in Send trait, use usize here.
    /// Store pthread's Key-Value.
    kv: RwLock<BTreeMap<pthread_key_t, usize>>,
    stack_start: usize,
    // 0 indicates joinable, 1 indicates detached. -1 indicates the state is frozen and is set in pthread_exit.
    detached: AtomicI8,
    waitval: Waitval<usize>,
}

#[inline(always)]
fn get_tcb(tid: pthread_t) -> Option<Arc<RwLock<PthreadTcb>>> {
    TCBS.read().get(&tid).map(|tcb| Arc::clone(tcb))
}

#[inline(always)]
fn get_my_tcb() -> Option<Arc<RwLock<PthreadTcb>>> {
    let tid = pthread_self();
    get_tcb(tid)
}

fn drop_my_tcb() {
    let tid = pthread_self();
    drop_tcb(tid);
}

fn drop_tcb(tid: pthread_t) {
    let mut lock = TCBS.write();
    lock.remove(&tid);
}

// Prefer using C ABI here since it's stablized.
// FIXME: Alignment should be target dependent.
#[repr(C, align(4))]
struct PosixRoutineInfo {
    pub entry: extern "C" fn(arg: *mut c_void) -> *mut c_void,
    pub arg: *mut c_void,
}

extern "C" fn posix_start_routine(arg: *mut c_void) {
    let routine = arg.cast::<PosixRoutineInfo>();
    let retval = unsafe { ((*routine).entry)((*routine).arg) };
    pthread_exit(retval);
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_attr_init(attr: *mut pthread_attr_t) -> c_int {
    let inner_attr = attr as *mut InnerPthreadAttr;
    unsafe {
        (*inner_attr).stack_size = DEFAULT_STACK_SIZE;
    }
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_attr_destroy(_: *mut pthread_attr_t) -> c_int {
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_attr_setstacksize(
    attr: *mut pthread_attr_t,
    stacksize: c_size_t,
) -> c_int {
    unsafe { (*(attr as *mut InnerPthreadAttr)).stack_size = stacksize };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_self() -> pthread_t {
    bk_syscall!(GetTid) as pthread_t
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_detach(t: pthread_t) -> c_int {
    let Some(tcb) = get_tcb(t) else {
        return ESRCH;
    };
    let old_val = tcb
        .read()
        .detached
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed);
    if let Err(failed_val) = old_val {
        if failed_val == -1 {
            return EINVAL;
        } else {
            return 0;
        }
    } else {
        return 0;
    }
}

fn register_posix_tcb(tid: usize, clone_args: &CloneArgs) {
    let tid = tid as pthread_t;
    {
        let tcb = Arc::new(RwLock::new(PthreadTcb {
            kv: RwLock::new(BTreeMap::new()),
            stack_start: clone_args.stack_start as usize,
            detached: AtomicI8::new(0),
            waitval: Waitval::new(),
        }));
        let mut write = TCBS.write();
        write.insert(tid, tcb);
    }
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_create(
    thread: *mut pthread_t,
    attr: *const pthread_attr_t,
    start_routine: PosixRoutineEntry,
    arg: *mut c_void,
) -> c_int {
    let stack_size = if attr.is_null() {
        DEFAULT_STACK_SIZE
    } else {
        unsafe { (*(attr as *const InnerPthreadAttr)).stack_size }
    };
    // We'll put PosixRoutineInfo on stack.
    let size = stack_size + core::mem::size_of::<PosixRoutineInfo>();
    let mut stack_start: *mut u8 = core::ptr::null_mut();
    unsafe { posix_memalign(&mut stack_start as *mut *mut u8, STACK_ALIGN, size) };
    assert!(!stack_start.is_null());
    let posix_routine_info_ptr = unsafe { stack_start.offset(stack_size as isize) as *mut c_void };
    assert_eq!(
        posix_routine_info_ptr.align_offset(core::mem::align_of::<PosixRoutineInfo>()),
        0
    );
    let posix_routine_info = unsafe { &mut *(posix_routine_info_ptr as *mut PosixRoutineInfo) };
    posix_routine_info.entry = start_routine;
    posix_routine_info.arg = arg;
    let clone_args = CloneArgs {
        clone_hook: Some(register_posix_tcb),
        entry: posix_start_routine,
        arg: posix_routine_info_ptr,
        stack_start: stack_start,
        stack_size: stack_size,
    };
    let tid = bk_syscall!(CreateThread, &clone_args as *const CloneArgs) as c_int;
    if tid == -1 {
        unsafe { free(stack_start) };
        return -1;
    }
    unsafe { *thread = tid as pthread_t };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_join(tid: pthread_t, retval: *mut *mut c_void) -> c_int {
    if tid == pthread_self() {
        return EDEADLK;
    }
    let Some(tcb) = get_tcb(tid) else {
        return ESRCH;
    };
    let val = tcb.read().waitval.wait().clone();
    if !retval.is_null() {
        unsafe { *retval = transmute::<usize, *mut c_void>(val) };
    }
    drop_tcb(tid);
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_exit(retval: *mut c_void) -> ! {
    // pthread_detach must fail if tcb.state != RUNNING.
    let tid = pthread_self();
    let Some(tcb) = get_tcb(tid) else {
        panic!("{:x}: My tcb is gone!", tid)
    };
    let detached = tcb.read().detached.swap(-1, Ordering::SeqCst);
    assert_ne!(detached, -1, "pthread_exit should be only called once");
    if detached == 0 {
        tcb.read().waitval.post(retval as usize);
    }
    // We have to cleanup all resources allocated.
    {
        let read_tcb = tcb.read();
        let read_tcb_kv = read_tcb.kv.read();
        // We have to collect dtors and vals first, since some dtors might write KEYS.
        let mut dtors = Vec::new();
        let mut vals = Vec::new();
        for (key, val) in read_tcb_kv.iter() {
            let keys = KEYS.read();
            if let Some(dtor) = keys.get(key) {
                let ptr: *mut c_void = unsafe { transmute::<usize, *mut c_void>(*val) };
                dtor.0.as_ref().map(|f| {
                    dtors.push(*f);
                    vals.push((*key, ptr));
                });
            }
        }
        drop(read_tcb_kv);
        drop(read_tcb);
        for i in 0..dtors.len() {
            dtors[i](vals[i].1);
        }
    }
    unsafe {
        free(transmute::<usize, *mut u8>(tcb.read().stack_start));
    }
    // pthread_join should drop the tcb otherwise.
    if detached == 1 {
        drop_my_tcb();
    }
    bk_syscall!(ExitThread);
    unreachable!("We have called system call to exit this thread, so should not reach here");
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_setspecific(key: pthread_key_t, val: *const c_void) -> c_int {
    if !KEYS.read().contains_key(&key) {
        return EINVAL;
    }
    let tid = pthread_self();
    let Some(tcb) = get_tcb(tid) else {
        panic!("{:x}: My tcb is gone!", tid)
    };
    tcb.read().kv.write().insert(key, val as usize);
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_getspecific(key: pthread_key_t) -> *mut c_void {
    if !KEYS.read().contains_key(&key) {
        return core::ptr::null_mut();
    }
    let tid = pthread_self();
    let Some(tcb) = get_tcb(tid) else {
        panic!("{:x}: My tcb is gone!", tid)
    };
    {
        let read_tcb = tcb.read();
        let read_tcb_kv = read_tcb.kv.read();
        let Some(val) = read_tcb_kv.get(&key) else {
            return core::ptr::null_mut();
        };
        let val = *val;
        drop(read_tcb_kv);
        drop(read_tcb);
        unsafe { transmute::<usize, *mut c_void>(val) }
    }
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_set_name_np(t: pthread_t, name: *const i8) -> c_int {
    // TODO
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_condattr_init(condattr: *mut pthread_condattr_t) -> c_int {
    unsafe { condattr.cast::<CondAttr>().write(CondAttr::default()) };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_condattr_destroy(condattr: *mut pthread_condattr_t) -> c_int {
    unsafe { core::ptr::drop_in_place(condattr.cast::<CondAttr>()) };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_condattr_setclock(
    condattr: *mut pthread_condattr_t,
    clock: clockid_t,
) -> c_int {
    unsafe { (*condattr.cast::<CondAttr>()).clock = clock };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_cond_init(
    cond: *mut pthread_cond_t,
    _attr: *const pthread_condattr_t,
) -> c_int {
    unsafe { cond.cast::<Cond>().write(Cond::new()) };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_cond_signal(cond: *mut pthread_cond_t) -> c_int {
    unsafe { (&*cond.cast::<Cond>()).signal() }.map_or_else(|e| e, |_| 0)
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_cond_destroy(cond: *mut pthread_cond_t) -> c_int {
    unsafe { core::ptr::drop_in_place(cond.cast::<Cond>()) };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_cond_wait(
    cond: *mut pthread_cond_t,
    mutex: *mut pthread_mutex_t,
) -> c_int {
    unsafe { (&*cond.cast::<Cond>()).wait(&*mutex.cast::<&Mutex>()) }.map_or_else(|e| e, |_| 0)
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_key_create(
    key: *mut pthread_key_t,
    dtor: Option<extern "C" fn(_: *mut c_void)>,
) -> c_int {
    let tid = pthread_self();
    let new_key = KEY_COUNTER.fetch_add(1, Ordering::Relaxed) as pthread_key_t;
    let mut lock = KEYS.write();
    lock.insert(new_key, Dtor(dtor));
    drop(lock);
    unsafe {
        *key = new_key;
    }
    0
}

// We expect user to have released resources bound to this key in all threads before
// calling this function.
#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_key_delete(key: pthread_key_t) -> c_int {
    let tid = pthread_self();
    KEYS.write().remove(&key);
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutexattr_init(attr: *mut pthread_mutexattr_t) -> c_int {
    unsafe { attr.cast::<MutexAttr>().write(MutexAttr::default()) };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutexattr_destroy(attr: *mut pthread_mutexattr_t) -> c_int {
    unsafe { core::ptr::drop_in_place(attr.cast::<MutexAttr>()) };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutexattr_settype(attr: *mut pthread_mutexattr_t, ty: c_int) -> c_int {
    unsafe { (*attr.cast::<MutexAttr>()).ty = ty };
    0
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutex_init(
    mutex: *mut pthread_mutex_t,
    attr: *const pthread_mutexattr_t,
) -> c_int {
    let attr = unsafe {
        attr.cast::<MutexAttr>()
            .as_ref()
            .copied()
            .unwrap_or_default()
    };
    Mutex::new(&attr).map_or_else(
        |e| e,
        |new| {
            unsafe { mutex.cast::<Mutex>().write(new) };
            0
        },
    )
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutex_lock(mutex: *mut pthread_mutex_t) -> c_int {
    unsafe { (&*mutex.cast::<Mutex>()).lock() }.map_or_else(|e| e, |_| 0)
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t) -> c_int {
    unsafe { (&*mutex.cast::<Mutex>()).unlock() }.map_or_else(|e| e, |_| 0)
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> c_int {
    unsafe { (&*mutex.cast::<Mutex>()).try_lock() }.map_or_else(|e| e, |_| 0)
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t) -> c_int {
    unsafe { core::ptr::drop_in_place(mutex.cast::<Mutex>()) };
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    macro_rules! check_align {
        ($lhs:ident, $rhs:ident) => {
            assert_eq!(align_of::<$lhs>(), align_of::<$rhs>())
        };
    }

    macro_rules! check_size {
        ($lhs:ident, $rhs:ident) => {
            assert_eq!(size_of::<$lhs>(), size_of::<$rhs>())
        };
    }

    #[test_case]
    fn check_type_consistency() {
        check_align!(pthread_mutex_t, Mutex);
        check_size!(pthread_mutex_t, Mutex);
        check_align!(pthread_mutexattr_t, MutexAttr);
        check_size!(pthread_mutexattr_t, MutexAttr);
        check_align!(pthread_cond_t, Cond);
        check_size!(pthread_cond_t, Cond);
        check_align!(usize, pthread_t);
        check_size!(usize, pthread_t);
        check_align!(pthread_attr_t, InnerPthreadAttr);
        check_size!(pthread_attr_t, InnerPthreadAttr);
        check_align!(pthread_condattr_t, CondAttr);
        check_size!(pthread_condattr_t, CondAttr);
    }
}
