use crate::{
    sync::SpinLockGuard,
    thread::ThreadNode,
    types::{impl_simple_intrusive_adapter, ArcList, IlistHead},
};

impl_simple_intrusive_adapter!(OffsetOfWait, WaitEntry, wait_node);

pub type WaitQueue = ArcList<WaitEntry, OffsetOfWait>;

#[derive(Debug)]
pub struct WaitEntry {
    pub wait_node: IlistHead<WaitEntry, OffsetOfWait>,
    pub thread: ThreadNode,
}

impl !Send for WaitEntry {}
impl !Sync for WaitEntry {}

pub(crate) struct WaitQueueGuardDropper<'a, const N: usize> {
    guards: [Option<SpinLockGuard<'a, WaitQueue>>; N],
    num_active_guards: usize,
}

impl<'a, const N: usize> WaitQueueGuardDropper<'a, N> {
    pub const fn const_new() -> Self {
        Self {
            guards: [const { None }; N],
            num_active_guards: 0,
        }
    }

    pub const fn new() -> Self {
        Self::const_new()
    }

    #[inline]
    pub fn add(&mut self, w: SpinLockGuard<'a, WaitQueue>) -> bool {
        if self.num_active_guards == N {
            return false;
        }
        assert!(self.guards[self.num_active_guards].is_none());
        self.guards[self.num_active_guards] = Some(w);
        self.num_active_guards += 1;
        return true;
    }
}

pub(crate) type DefaultWaitQueueGuardDropper<'a> = WaitQueueGuardDropper<'a, 2>;
