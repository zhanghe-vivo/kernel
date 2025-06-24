use crate::arch::aarch64::{
    asm,
    asm::DsbOptions,
    registers::{mair_el1::*, sctlr_el1::*, tcr_el1::*, ttbr0_el1::TTBR0_EL1},
};
use tock_registers::{interfaces::*, register_bitfields, registers::InMemoryRegister};

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum MemAttributes {
    Device = 0,
    Normal = 1,
}

register_bitfields! {u64,
    pub PAGE_DESCRIPTOR [
        /// Unprivileged execute-never.
        UXN OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Privileged execute-never.
        PXN OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Current page table entry is in a continuous set of physical pages
        CONT_PHY_PAGES OFFSET(52) NUMBITS(1) [],

        /// Indicates that the page has been modified
        DMB OFFSET(51) NUMBITS(1) [],

        /// Output physical address
        OUTPUT_ADDR OFFSET(16) NUMBITS(36) [],

        /// When this bit is set, it means that the TLB page table entry corresponding to this page is process-specific.
        NG OFFSET(11) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Access flag, The hardware will automatically set up when you visit the page for the first time
        AF OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        /// Shared Memory Attributes
        SH OFFSET(8) NUMBITS(2) [
            NotShareable = 0b00,
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        /// Date access Permissions.
        /// AP[1] :
        ///   0: not accessible via EL0, but accessible via EL1
        ///   1: accessed through EL0 and higher privilege exceptions
        /// AP[2] :
        ///   0: read only
        ///   1: read and write
        AP OFFSET(6) NUMBITS(2) [
            EL1_RW = 0b00,
            EL0_RW = 0b01,
            EL1_RO = 0b10,
            EL0_RO = 0b11
        ],

        /// Non-secure
        NS OFFSET(5) NUMBITS(1) [],

        /// Memory attributes index.
        ATTRINDX OFFSET(2) NUMBITS(3) [],

        /// 0: Reserved page table entries
        /// 1: Page table
        TYPE OFFSET(1) NUMBITS(1) [
            Reserved = 0,
            Page = 1
        ],

        /// 0: Invalid descriptor
        /// 1: valid descriptor
        VALID OFFSET(0) NUMBITS(1) [
            Invalid = 0,
            Valid = 1
        ]
    ]
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct PageEntry(u64);

impl PageEntry {
    // Create a new invalid entry
    const fn new() -> Self {
        Self(0)
    }

    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    // Set page entry
    fn set(&mut self, output_addr: u64, attributes: MemAttributes) -> Result<(), &'static str> {
        if self.is_valid() {
            return Err("page entry is set");
        }
        let entry = InMemoryRegister::<u64, PAGE_DESCRIPTOR::Register>::new(0);
        match attributes {
            MemAttributes::Device => entry.write(
                PAGE_DESCRIPTOR::VALID::Valid
                    + PAGE_DESCRIPTOR::AF::True
                    + PAGE_DESCRIPTOR::ATTRINDX.val(MemAttributes::Device as u64)
                    + PAGE_DESCRIPTOR::UXN::True,
            ),
            MemAttributes::Normal => entry.write(
                PAGE_DESCRIPTOR::VALID::Valid
                    + PAGE_DESCRIPTOR::AF::True
                    + PAGE_DESCRIPTOR::ATTRINDX.val(MemAttributes::Normal as u64)
                    + PAGE_DESCRIPTOR::SH::InnerShareable
                    + PAGE_DESCRIPTOR::NG::True,
            ),
        }
        self.0 = entry.get() | output_addr;
        Ok(())
    }

    // Check the entry is valid
    fn is_valid(&self) -> bool {
        InMemoryRegister::<u64, PAGE_DESCRIPTOR::Register>::new(self.0)
            .is_set(PAGE_DESCRIPTOR::VALID)
    }
}

#[used]
static mut TABLE_MANAGER: PageTableManager = PageTableManager::new();

#[repr(C, align(4096))]
pub struct PageTableManager([PageEntry; 512]);

impl PageTableManager {
    const fn new() -> Self {
        PageTableManager([PageEntry::new(); 512])
    }

    fn init() {
        let table = unsafe { &mut TABLE_MANAGER };
        // 1 GiB of device memory.
        table.0[0].set(0x0, MemAttributes::Device);
        // 1 GiB of normal memory.
        table.0[1].set(0x40080000, MemAttributes::Normal);
    }
}

pub fn enable_mmu() {
    PageTableManager::init();
    // Set physical table base addr.
    unsafe {
        core::arch::asm!(
            "adrp {temp}, {tbl}",
            "msr ttbr0_el1, {temp}",
            temp = reg(out) _,
            tbl = sym TABLE_MANAGER,
            options(nostack, nomem)
        )
    };
    // Set memory type.
    MAIR_EL1.write(
        MAIR_EL1::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
            + MAIR_EL1::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc
            + MAIR_EL1::Attr0_Device::NonGathering_NonReordering_EarlyWriteAck,
    );
    // Configure address translation related control information.
    TCR_EL1.write(
        TCR_EL1::TBI0::Used
            + TCR_EL1::IPS::Bits_32
            + TCR_EL1::TG0::KiB_4
            + TCR_EL1::SH0::InnerShareable
            + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::EPD1::DisableTTBR1Walks
            + TCR_EL1::EPD0::EnableTTBR0Walks
            + TCR_EL1::T0SZ.val(25),
    );
    // Clear tlb.
    asm::tlbi_all();
    asm::dsb(DsbOptions::Sys);
    asm::isb_sy();
    // Enable the MMU.
    SCTLR_EL1.modify(
        SCTLR_EL1::M::Enable
            + SCTLR_EL1::C::Cacheable
            + SCTLR_EL1::I::Cacheable
            + SCTLR_EL1::SA::Enable,
    );
    asm::isb_sy();
}
