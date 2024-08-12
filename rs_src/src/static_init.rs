use crate::sync::RawSpin;
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

pub struct UnsafeStaticInit<T, I> {
    pub cell: UnsafeCell<MaybeUninit<T>>,
    init: Cell<Option<I>>,
    present: Cell<bool>,
}

unsafe impl<T: Sync, I> Sync for UnsafeStaticInit<T, I> {}
unsafe impl<T: Send, I> Send for UnsafeStaticInit<T, I> {}

impl<T, I: PinInit<T>> UnsafeStaticInit<T, I> {
    pub const fn new(init: I) -> Self {
        Self {
            cell: UnsafeCell::new(MaybeUninit::uninit()),
            init: Cell::new(Some(init)),
            present: Cell::new(false),
        }
    }

    #[inline]
    pub fn is_inited(&self) -> bool {
        self.present.get()
    }

    pub fn init_once(&self) {
        debug_assert!(!self.is_inited());
        let ptr = self.cell.get().cast::<T>();
        match self.init.take() {
            Some(f) => unsafe { f.__pinned_init(ptr).unwrap() },
            None => unsafe { core::hint::unreachable_unchecked() },
        }
        self.present.set(true);
    }
}

impl<T, I: PinInit<T>> ops::Deref for UnsafeStaticInit<T, I> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        debug_assert!(self.is_inited());
        unsafe { (*self.cell.get()).assume_init_ref() }
    }
}

impl<T, I: PinInit<T>> ops::DerefMut for UnsafeStaticInit<T, I> {
    fn deref_mut(&mut self) -> &mut T {
        debug_assert!(self.is_inited());
        unsafe { (*self.cell.get()).assume_init_mut() }
    }
}

// rewrite as OnceCell. with thread wait.
pub struct StaticInit<T, I> {
    inner: UnsafeStaticInit<T, I>,
    lock: RawSpin,
}

unsafe impl<T: Sync, I> Sync for StaticInit<T, I> {}
unsafe impl<T: Send, I> Send for StaticInit<T, I> {}

impl<T, I: PinInit<T>> StaticInit<T, I> {
    pub const fn new(init: I) -> Self {
        Self {
            inner: UnsafeStaticInit::new(init),
            lock: RawSpin::new(),
        }
    }
}

impl<T, I: PinInit<T>> ops::Deref for StaticInit<T, I> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        if self.inner.is_inited() {
            unsafe { (*self.inner.cell.get()).assume_init_ref() }
        } else {
            let _guard = self.lock.acquire();
            // check again.
            if self.inner.is_inited() {
                return unsafe { (*self.inner.cell.get()).assume_init_ref() };
            }
            // "doing init";
            self.inner.init_once();
            unsafe { (*self.inner.cell.get()).assume_init_ref() }
        }
    }
}

impl<T, I: PinInit<T>> ops::DerefMut for StaticInit<T, I> {
    fn deref_mut(&mut self) -> &mut T {
        if self.inner.is_inited() {
            unsafe { (*self.inner.cell.get()).assume_init_mut() }
        } else {
            let _guard = self.lock.acquire();
            // check again.
            if self.inner.is_inited() {
                return unsafe { (*self.inner.cell.get()).assume_init_mut() };
            }
            // "doing init";
            self.inner.init_once();
            unsafe { (*self.inner.cell.get()).assume_init_mut() }
        }
    }
}
