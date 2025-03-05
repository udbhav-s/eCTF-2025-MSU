MEMORY 
{
    ROM         (rx) : ORIGIN = 0x00000000, LENGTH = 0x00010000 /* 64kB ROM */
    BOOTLOADER  (rx) : ORIGIN = 0x10000000, LENGTH = 0x0000E000 /* Bootloader flash */
    FLASH       (rx) : ORIGIN = 0x1000E000, LENGTH = 0x00070000 /* Location of team firmware */
    RESERVED    (rw) : ORIGIN = 0x1007E000, LENGTH = 0x0001C000 /* Reserved */
    ROM_BL_PAGE (rw) : ORIGIN = 0x1007E000, LENGTH = 0x00002000 /* Reserved */
    RAM         (rwx): ORIGIN = 0x20000000, LENGTH = 0x00010000 /* 64kB RAM */
}

_stext = ORIGIN(FLASH) + 0x200; /* Jump point for bootloader */

SECTIONS {
    .flash_code :
    {
        . = ALIGN(4);
        *(.flashprog*)
        . = ALIGN(4);
    } > RAM AT>FLASH
}

INSERT AFTER .data;

/* This is where the call stack will be allocated. */
/* The stack is of the full descending type. */
/* You may want to use this variable to locate the call stack and static
   variables in different memory regions. Below is shown the default value */
/* _stack_start = ORIGIN(RAM) + LENGTH(RAM); */

/* You can use this symbol to customize the location of the .text section */
/* If omitted the .text section will be placed right after the .vector_table
   section */
/* This is required only on microcontrollers that store some configuration right
   after the vector table */
/* _stext = ORIGIN(FLASH) + 0x400; */

/* Example of putting non-initialized variables into custom RAM locations. */
/* This assumes you have defined a region RAM2 above, and in the Rust
   sources added the attribute `#[link_section = ".ram2bss"]` to the data
   you want to place there. */
/* Note that the section will not be zero-initialized by the runtime! */
/* SECTIONS {
     .ram2bss (NOLOAD) : ALIGN(4) {
       *(.ram2bss);
       . = ALIGN(4);
     } > RAM2
   } INSERT AFTER .bss;
*/
