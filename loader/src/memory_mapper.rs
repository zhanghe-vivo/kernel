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

extern crate alloc;
use alloc::{sync::Arc, vec};

#[derive(Debug)]
pub struct MemoryMapper {
    entry: usize,
    start: usize,
    end: usize,
    mem: Option<Arc<[u8]>>,
    align: usize,
}

impl MemoryMapper {
    #[inline]
    pub fn new() -> Self {
        Self {
            entry: 0,
            start: usize::MAX,
            end: 0,
            mem: None,
            #[cfg(target_arch = "aarch64")]
            align: 4096,
            #[cfg(not(target_arch = "aarch64"))]
            align: core::mem::size_of::<usize>(),
        }
    }

    #[inline]
    pub fn entry(&self) -> usize {
        self.entry
    }

    #[inline]
    pub fn set_entry(&mut self, entry: usize) -> &mut Self {
        self.entry = entry;
        self
    }

    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    #[inline]
    pub fn update_start(&mut self, val: usize) -> &mut Self {
        if val < self.start {
            self.start = val;
        }
        self
    }

    #[inline]
    pub fn update_end(&mut self, val: usize) -> &mut Self {
        if val > self.end {
            self.end = val;
        }
        self
    }

    #[inline]
    pub fn total_size(&self) -> usize {
        self.end - self.start
    }

    #[inline]
    pub fn allocate_memory(&mut self) -> Arc<[u8]> {
        // FIXME: We are not using paging yet, so alignment(usually
        // 4096) specified in program header is not applied here.
        let mem: Arc<[u8]> = Arc::from(vec![0u8; self.total_size() + self.align]);
        self.mem = Some(mem.clone());
        mem
    }

    #[inline]
    pub fn real_start(&self) -> Option<*const u8> {
        self.mem.as_ref().map(|mem| {
            let p = mem.as_ptr();
            return unsafe { p.offset(p.align_offset(self.align) as isize) };
        })
    }

    #[inline]
    pub fn real_start_mut(&self) -> Option<*mut u8> {
        self.mem.as_ref().map(|mem| {
            let p = mem.as_ptr() as *mut u8;
            return unsafe { p.offset(p.align_offset(self.align) as isize) };
        })
    }

    #[inline]
    pub fn real_entry(&self) -> Option<*const u8> {
        self.mem.as_ref().map(|_| {
            let offset = (self.entry - self.start) as isize;
            unsafe { self.real_start().unwrap().offset(offset) }
        })
    }

    #[inline]
    pub fn memory(&self) -> Option<Arc<[u8]>> {
        self.mem.clone()
    }
}
