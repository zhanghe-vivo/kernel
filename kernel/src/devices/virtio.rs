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

// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::devices::block::init_virtio_block;
use alloc::{
    alloc::{alloc_zeroed, dealloc, handle_alloc_error},
    vec::Vec,
};
use core::{alloc::Layout, mem::size_of, ptr::NonNull};
use flat_device_tree::{node::FdtNode, Fdt};
use log::{debug, error};
use spin::{Once, RwLock};
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{
        mmio::{MmioError, MmioTransport, VirtIOHeader},
        DeviceType, DeviceTypeError, SomeTransport, Transport,
    },
    BufferDirection, Hal, PhysAddr, PAGE_SIZE,
};

const VIRTIO_MMIO_COMPATIBLE: &str = "virtio,mmio";

pub fn init_virtio(fdt: &Fdt) {
    unsafe { find_virtio_mmio_devices(fdt) };
}

/// # Safety
///
/// Any VirtIO MMIO devices in the given device tree must exist and be mapped appropriately, and
/// must not be constructed anywhere else.
pub unsafe fn find_virtio_mmio_devices(fdt: &Fdt) {
    for node in fdt.all_nodes() {
        if is_compatible(&node, &[VIRTIO_MMIO_COMPATIBLE]) {
            debug!("Found VirtIO MMIO device {}", node.name);
            if let Some(region) = node.reg().next() {
                let region_size = region.size.unwrap_or(0);
                if region_size < size_of::<VirtIOHeader>() {
                    debug!(
                        "VirtIO MMIO device {} region smaller than VirtIO header size ({} < {})",
                        node.name,
                        region_size,
                        size_of::<VirtIOHeader>()
                    );
                } else {
                    let header =
                        NonNull::new(region.starting_address as *mut VirtIOHeader).unwrap();
                    // SAFETY: The caller promised that the device tree is correct, VirtIO MMIO
                    // devices are mapped, and no aliases are constructed to the MMIO region.
                    match unsafe { MmioTransport::new(header, region_size) } {
                        Err(MmioError::InvalidDeviceID(DeviceTypeError::InvalidDeviceType(0))) => {
                            debug!("Ignoring VirtIO device with zero device ID.");
                        }
                        Err(e) => {
                            debug!("Error creating VirtIO transport: {}", e);
                        }
                        Ok(mut transport) => {
                            debug!(
                                "Detected virtio MMIO device with device type {:?}, vendor ID {:#x}, version {:?}, features {:#018x}",
                                transport.device_type(),
                                transport.vendor_id(),
                                transport.version(),
                                transport.read_device_features(),
                            );
                            init_virtio_device(transport.into());
                        }
                    }
                }
            } else {
                debug!("VirtIO MMIO device {} missing region", node.name);
            }
        }
    }
}

fn is_compatible(node: &FdtNode, with: &[&str]) -> bool {
    if let Some(compatible) = node.compatible() {
        compatible.all().any(|c| with.contains(&c))
    } else {
        false
    }
}

fn init_virtio_device(transport: SomeTransport<'static>) {
    match transport.device_type() {
        DeviceType::Network => {}
        DeviceType::Block => {
            if let Err(e) = init_virtio_block(VirtIOBlk::new(transport).unwrap()) {
                error!("Failed to init virtio blk, {:?}", e);
            }
        }
        DeviceType::Console => {}
        t => {
            debug!("Ignoring unsupported VirtIO device type {:?}", t);
        }
    }
}

#[derive(Debug)]
pub struct VirtioHal;

// SAFETY: dma_alloc and mmio_phys_to_virt always return appropriate pointers based on their
// parameters.
unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        assert_ne!(pages, 0);
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        // SAFETY: The layout has a non-zero size because we just checked that `pages` is non-zero.
        let vaddr = unsafe { alloc_zeroed(layout) };
        let vaddr = if let Some(vaddr) = NonNull::new(vaddr) {
            vaddr
        } else {
            handle_alloc_error(layout)
        };
        let paddr = virt_to_phys(vaddr.as_ptr() as _);
        (paddr, vaddr)
    }

    unsafe fn dma_dealloc(_paddr: PhysAddr, vaddr: NonNull<u8>, pages: usize) -> i32 {
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        // SAFETY: the memory was allocated by `dma_alloc` above using the same allocator, and the
        // layout is the same as was used then.
        unsafe {
            dealloc(vaddr.as_ptr(), layout);
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as _).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        // Nothing to do, as the host already has access to all memory.
        virt_to_phys(vaddr)
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Nothing to do, as the host already has access to all memory and we didn't copy the buffer
        // anywhere else.
    }
}

fn virt_to_phys(vaddr: usize) -> PhysAddr {
    vaddr
}
