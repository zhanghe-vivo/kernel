use crate::rt_bindings::rt_spinlock;
use core::{
    cell::{Cell, UnsafeCell},
    mem::MaybeUninit,
    ops,
};
use pinned_init::PinInit;

// # Examples
//
// ```rust,ignore
// pub struct CountInit;

// unsafe impl PinInit<Mutex<usize>> for CountInit {
//     unsafe fn __pinned_init(
//         self,
//         slot: *mut Mutex<usize>,
//     ) -> Result<(), core::convert::Infallible> {
//         let init = Mutex::new(0);
//         unsafe { init.__pinned_init(slot) }
//     }
// }

// pub static COUNT: StaticInit<Mutex<usize>, CountInit> = StaticInit::new(CountInit);
// ```

pub struct StaticInit<T, I> {
    cell: UnsafeCell<MaybeUninit<T>>,
    init: Cell<Option<I>>,
    lock: rt_spinlock,
    present: Cell<bool>,
}

unsafe impl<T: Sync, I> Sync for StaticInit<T, I> {}
unsafe impl<T: Send, I> Send for StaticInit<T, I> {}

impl<T, I: PinInit<T>> StaticInit<T, I> {
    pub const fn new(init: I) -> Self {
        Self {
            cell: UnsafeCell::new(MaybeUninit::uninit()),
            init: Cell::new(Some(init)),
            lock: rt_spinlock::new(),
            present: Cell::new(false),
        }
    }
}

impl<T, I: PinInit<T>> ops::Deref for StaticInit<T, I> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        if self.present.get() {
            unsafe { (*self.cell.get()).assume_init_ref() }
        } else {
            let _guard = self.lock.acquire();
            if self.present.get() {
                return unsafe { (*self.cell.get()).assume_init_ref() };
            }
            // "doing init";
            let ptr = self.cell.get().cast::<T>();
            match self.init.take() {
                Some(f) => unsafe { f.__pinned_init(ptr).unwrap() },
                None => unsafe { core::hint::unreachable_unchecked() },
            }
            self.present.set(true);
            unsafe { (*self.cell.get()).assume_init_ref() }
        }
    }
}
