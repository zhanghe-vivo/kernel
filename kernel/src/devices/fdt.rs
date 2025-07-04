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

use crate::println;
use flat_device_tree::Fdt;
use spin::Once;

static FDT: Once<Fdt<'static>> = Once::new();

pub fn fdt_init(base: u64) {
    println!("FDT address: 0x{:x}", base);
    // SAFETY: We trust that the FDT pointer we were given is valid, and this is the only time we
    // use it.
    let fdt = unsafe { Fdt::from_ptr(base as *const u8).unwrap() };
    println!("FDT size: {} bytes", fdt.total_size());
    println!("FDT: {:?}", fdt);
    for reserved in fdt.memory_reservations() {
        println!("Reserved memory: {:?}", reserved);
    }
    FDT.call_once(|| fdt);
}

pub fn get_fdt() -> &'static Fdt<'static> {
    FDT.get().unwrap()
}
