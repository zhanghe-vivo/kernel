OUTPUT_FORMAT("elf64-littleaarch64", "elf64-littleaarch64", "elf64-littleaarch64")
OUTPUT_ARCH(aarch64)

STACK_SIZE = 64 * 1024;

MEMORY
{
	/* dram ORIGIN start addr need to bigger than ram_base + fdt_size */
	dram : ORIGIN = 0x40100000, LENGTH = 2M
}

PHDRS
{
  /* R = 100, W = 010, X = 001 */

  text   PT_LOAD FLAGS(5); /* RX */
  rodata PT_LOAD FLAGS(4); /* R  */
  data   PT_LOAD FLAGS(6); /* RW */
}

ENTRY(_start)
SECTIONS
{
    .text :
    {
        __text_start = .;
        _start = .;
        KEEP(*(.text._start))
        KEEP(*(.text._startup_el1))
        KEEP(*(.text.vector_table))
        KEEP(*(.text._exception))
        *(.text*)
        __text_end = .;
    } > dram :text

    .rodata : ALIGN(4096)
    {
        __rodata_start = .;
        *(.rodata*)
        __rodata_end = .;
    } > dram :rodata

    .data : ALIGN(4096)
    {
        __data_start = .;
        *(.data*)
        __data_end = .;
    } > dram :data

    .bss : ALIGN(4096)
    {
        __bss_start = .;
        *(.bss*)
        __bss_end = .;
    } > dram :data

    .stack : ALIGN(4096)
    {
        __sys_stack_start = .;
        . += STACK_SIZE;
        __sys_stack_end = .;
    } > dram :data

    . = ALIGN(4096);
    __heap_start = .;
    _end = .;

    .stab 0 : { *(.stab) }
    .stabstr 0 : { *(.stabstr) }
    .stab.excl 0 : { *(.stab.excl) }
    .stab.exclstr 0 : { *(.stab.exclstr) }
    .stab.index 0 : { *(.stab.index) }
    .stab.indexstr 0 : { *(.stab.indexstr) }
    .comment 0 : { *(.comment) }
}
