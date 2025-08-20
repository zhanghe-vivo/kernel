// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// The implementation is trying conforming to
// https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__PoolMgmt.html.

use alloc::boxed::Box;
use blueos::{allocator, irq, sync::Semaphore, time::WAITING_FOREVER, types::Arc};
use blueos_infra::{
    impl_simple_intrusive_adapter,
    list::typed_ilist::{ListHead, ListIterator},
};
use cmsis_os2::{
    osMemoryPoolAttr_t, osMemoryPoolId_t, osStatus_t, osStatus_t_osErrorISR,
    osStatus_t_osErrorParameter, osStatus_t_osErrorResource, osStatus_t_osOK,
};
use core::cell::UnsafeCell;

// FIXME: We haven't got builtin memory pool API in allocator, so we just use
// HEAP's public API to implement the memory pool. There might be performance degradation.

type BlockList = ListHead<Block, Node>;
type BlockListIterator = ListIterator<Block, Node>;

struct MemoryPoolInner {
    attr: osMemoryPoolAttr_t,
    block_size: usize,
    total_blocks: usize,
    free_blocks: usize,
    head: BlockList,
}

impl MemoryPoolInner {
    fn new(block_count: usize, block_size: usize) -> Self {
        Self {
            attr: unsafe { core::mem::zeroed() },
            block_size,
            total_blocks: block_count,
            free_blocks: 0,
            head: BlockList::new(),
        }
    }

    fn init(&mut self) {
        const ALIGN: usize = core::mem::align_of::<Block>();
        let size = core::mem::size_of::<Block>() + self.block_size;
        for _i in 0..self.total_blocks {
            let ptr = allocator::malloc_align(size, ALIGN);
            let block = unsafe { &mut *ptr.cast::<Block>() };
            block.node.prev = None;
            block.node.next = None;
            let ok = BlockList::insert_after(&mut self.head, &mut block.node);
            debug_assert!(ok);
            self.free_blocks += 1;
        }
    }
}

impl_simple_intrusive_adapter!(Node, Block, node);

#[repr(C)]
struct Block {
    node: BlockList,
}

impl Block {
    #[inline]
    fn base(&mut self) -> *mut u8 {
        let this = self as *mut _ as *mut u8;
        // unsafe { this.offset(core::mem::size_of::<Self>() as isize) }
        unsafe { this.add(core::mem::size_of::<Self>()) }
    }
}

struct MemoryPool {
    sema: Semaphore,
    inner: UnsafeCell<MemoryPoolInner>,
}

impl MemoryPool {
    pub fn new(block_count: usize, block_size: usize) -> Self {
        Self {
            sema: Semaphore::new(1),
            inner: UnsafeCell::new(MemoryPoolInner::new(block_count, block_size)),
        }
    }

    fn init(&mut self) {
        self.sema.init();
        let inner = unsafe { &mut *self.inner.get() };
        inner.init();
    }

    fn get_block_with_timeout(&self, ticks: usize) -> *mut core::ffi::c_void {
        if irq::is_in_irq() {
            return core::ptr::null_mut();
        }
        let mut ok = false;
        if ticks == 0 {
            ok = self.sema.try_acquire();
        } else {
            ok = self.sema.acquire_timeout(ticks);
        }
        if !ok {
            return core::ptr::null_mut();
        }
        let inner = unsafe { &mut *self.inner.get() };
        let mut it = BlockListIterator::new(&inner.head, None);
        let Some(mut node) = it.next() else {
            self.sema.release();
            return core::ptr::null_mut();
        };
        let block = unsafe { node.as_mut().owner_mut() };
        let ok = BlockList::detach(&mut block.node);
        debug_assert!(ok);
        debug_assert!(block.node.is_detached());
        inner.free_blocks -= 1;
        self.sema.release();
        let ptr = block.base();
        ptr as *mut core::ffi::c_void
    }

    fn put_block(&self, block: *mut core::ffi::c_void) -> bool {
        let base = block as *mut u8;
        let block_ptr = unsafe {
            block
                .offset(-(core::mem::size_of::<Block>() as isize))
                .cast::<Block>()
        };
        self.sema.acquire_notimeout();
        let inner = unsafe { &mut *self.inner.get() };
        let block = unsafe { &mut *block_ptr };
        let ok = BlockList::insert_after(&mut inner.head, &mut block.node);
        if ok {
            inner.free_blocks += 1;
        }
        self.sema.release();
        ok
    }

    fn clear(&self) {
        const ALIGN: usize = core::mem::align_of::<Block>();
        self.sema.acquire_notimeout();
        let inner = unsafe { &mut *self.inner.get() };
        let it = BlockListIterator::new(&inner.head, None);
        for mut block in it {
            let ok = BlockList::detach(unsafe { block.as_mut() });
            debug_assert!(ok);
            inner.free_blocks -= 1;
            inner.total_blocks -= 1;
            let ptr = block.as_ptr() as *mut u8;
            allocator::free_align(ptr, ALIGN);
        }
        self.sema.release();
    }

    fn total_blocks(&self) -> usize {
        let inner = unsafe { &*self.inner.get() };
        inner.total_blocks
    }

    fn free_blocks(&self) -> usize {
        let inner = unsafe { &*self.inner.get() };
        inner.free_blocks
    }

    fn block_size(&self) -> usize {
        let inner = unsafe { &*self.inner.get() };
        inner.block_size
    }

    fn copy_attr(&mut self, attr: &osMemoryPoolAttr_t) -> &mut Self {
        let inner = unsafe { &mut *self.inner.get() };
        let _ = core::mem::replace(&mut inner.attr, *attr);
        self
    }
}

impl !Send for MemoryPool {}
unsafe impl Sync for MemoryPool {}

pub unsafe extern "C" fn osMemoryPoolNew(
    block_count: u32,
    block_size: u32,
    attr: *const osMemoryPoolAttr_t,
) -> osMemoryPoolId_t {
    // TODO: Support custom memory management specified in attr.
    let mut boxed = Box::new(MemoryPool::new(block_count as usize, block_size as usize));
    boxed.init();
    if !attr.is_null() {
        boxed.copy_attr(&*attr);
    }
    Box::into_raw(boxed) as osMemoryPoolId_t
}

pub unsafe extern "C" fn osMemoryPoolAlloc(
    mp_id: osMemoryPoolId_t,
    timeout: u32,
) -> *mut core::ffi::c_void {
    if mp_id.is_null() {
        return core::ptr::null_mut();
    }
    let mp = &mut *(mp_id as *mut MemoryPool);
    mp.get_block_with_timeout(timeout as usize)
}

pub unsafe extern "C" fn osMemoryPoolFree(
    mp_id: osMemoryPoolId_t,
    block: *mut core::ffi::c_void,
) -> osStatus_t {
    if mp_id.is_null() || block.is_null() {
        return osStatus_t_osErrorParameter;
    }
    let mp = &mut *(mp_id as *mut MemoryPool);
    mp.put_block(block);
    osStatus_t_osOK
}

pub unsafe extern "C" fn osMemoryPoolGetCapacity(mp_id: osMemoryPoolId_t) -> u32 {
    if mp_id.is_null() {
        return 0;
    }
    let mp = &mut *(mp_id as *mut MemoryPool);
    mp.total_blocks() as u32
}

pub unsafe extern "C" fn osMemoryPoolGetBlockSize(mp_id: osMemoryPoolId_t) -> u32 {
    if mp_id.is_null() {
        return 0;
    }
    let mp = &mut *(mp_id as *mut MemoryPool);
    mp.block_size() as u32
}

pub unsafe extern "C" fn osMemoryPoolGetCount(mp_id: osMemoryPoolId_t) -> u32 {
    if mp_id.is_null() {
        return 0;
    }
    let mp = &mut *(mp_id as *mut MemoryPool);
    (mp.total_blocks() - mp.free_blocks()) as u32
}

pub unsafe extern "C" fn osMemoryPoolGetSpace(mp_id: osMemoryPoolId_t) -> u32 {
    if mp_id.is_null() {
        return 0;
    }
    let mp = &mut *(mp_id as *mut MemoryPool);
    mp.free_blocks() as u32
}

pub unsafe extern "C" fn osMemoryPoolDelete(mp_id: osMemoryPoolId_t) -> osStatus_t {
    // FIXME: Check osErrorSafetyClass condition.
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    let ptr = mp_id as *mut MemoryPool;
    if ptr.is_null() {
        return osStatus_t_osErrorParameter;
    }
    let mp = &mut *ptr;
    if mp.total_blocks() != mp.free_blocks() {
        return osStatus_t_osErrorResource;
    }
    mp.clear();
    drop(Box::from_raw(ptr));
    osStatus_t_osOK
}

#[cfg(test)]
mod tests {
    use super::*;
    use blueos::thread;
    use blueos_test_macro::test;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_mempool_basic() {
        let mut mp = MemoryPool::new(4, 512);
        mp.init();
        assert_eq!(mp.block_size(), 512);
        assert_eq!(mp.total_blocks(), 4);
        assert_eq!(mp.free_blocks(), 4);
        let block = mp.get_block_with_timeout(0);
        assert!(!block.is_null());
        assert_eq!(mp.free_blocks(), 3);
        mp.put_block(block);
        assert_eq!(mp.free_blocks(), 4);
        mp.clear();
        assert_eq!(mp.free_blocks(), 0);
        assert_eq!(mp.total_blocks(), 0);
    }

    #[test]
    fn test_c_mempool_basic() {
        unsafe {
            let mp_id = osMemoryPoolNew(4, 512, core::ptr::null());
            assert_eq!(osMemoryPoolGetBlockSize(mp_id), 512);
            assert_eq!(osMemoryPoolGetCapacity(mp_id), 4);
            assert_eq!(osMemoryPoolGetCount(mp_id), 0);
            assert_eq!(osMemoryPoolGetSpace(mp_id), 4);
            let block = osMemoryPoolAlloc(mp_id, 1024);
            assert!(!block.is_null());
            assert_eq!(osMemoryPoolDelete(mp_id), osStatus_t_osErrorResource);
            assert_eq!(osMemoryPoolFree(mp_id, block), osStatus_t_osOK);
            assert_eq!(osMemoryPoolDelete(mp_id), osStatus_t_osOK);
        }
    }

    #[test]
    fn test_mempool_few_threads() {
        let n = 4;
        let mut mp = Arc::new(MemoryPool::new(n, 1024));
        Arc::get_mut(&mut mp).unwrap().init();
        let counter = Arc::new(AtomicUsize::new(0));
        for i in 0..n {
            let mp = mp.clone();
            let counter = counter.clone();
            thread::spawn(move || {
                let block = mp.get_block_with_timeout(1024);
                assert!(!block.is_null());
                mp.put_block(block);
                counter.fetch_add(1, Ordering::Relaxed);
            });
        }
        loop {
            if counter.load(Ordering::Relaxed) == n {
                break;
            }
            core::hint::spin_loop();
        }
        mp.clear();
    }

    #[test]
    fn test_mempool_many_threads() {
        // Do NOT increase this value greater than 256 while
        // TinyArc on 32-bit has only 8 bits for RC.
        let n = 192;
        let mut mp = Arc::new(MemoryPool::new(n, 64));
        Arc::get_mut(&mut mp).unwrap().init();
        let counter = Arc::new(AtomicUsize::new(0));
        for i in 0..n {
            let mp = mp.clone();
            let counter = counter.clone();
            thread::spawn(move || loop {
                let block = mp.get_block_with_timeout(WAITING_FOREVER);
                if block.is_null() {
                    continue;
                }
                mp.put_block(block);
                counter.fetch_add(1, Ordering::Relaxed);
                break;
            });
        }
        loop {
            if counter.load(Ordering::Acquire) == n {
                break;
            }
            core::hint::spin_loop();
        }
        assert_eq!(counter.load(Ordering::Relaxed), n);
        mp.clear();
    }
}
