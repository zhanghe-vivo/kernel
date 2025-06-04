OUTPUT_FORMAT("elf64-littleaarch64", "elf64-littleaarch64", "elf64-littleaarch64")
OUTPUT_ARCH(aarch64)

STACK_SIZE = 64 * 1024;

MEMORY
{
	dram : ORIGIN = 0x40280000, LENGTH = 2M
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
    /* 
    .debug 0 : { *(.debug) }
    .line 0 : { *(.line) }
    .debug_srcinfo 0 : { *(.debug_srcinfo) }
    .debug_sfnames 0 : { *(.debug_sfnames) }
    .debug_aranges 0 : { *(.debug_aranges) }
    .debug_pubnames 0 : { *(.debug_pubnames) }
    .debug_info 0 : { *(.debug_info .gnu.linkonce.wi.*) }
    .debug_abbrev 0 : { *(.debug_abbrev) }
    .debug_line 0 : { *(.debug_line) }
    .debug_frame 0 : { *(.debug_frame) }
    .debug_str 0 : { *(.debug_str) }
    .debug_loc 0 : { *(.debug_loc) }
    .debug_macinfo 0 : { *(.debug_macinfo) }
    .debug_weaknames 0 : { *(.debug_weaknames) }
    .debug_funcnames 0 : { *(.debug_funcnames) }
    .debug_typenames 0 : { *(.debug_typenames) }
    .debug_varnames 0 : { *(.debug_varnames) }
    */
}