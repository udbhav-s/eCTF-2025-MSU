target extended-remote :3333

# Load python utility to read flash control register
source ./debugging/flashctrl.py

# Debugging code for flash memory

# print demangled symbols
set print asm-demangle on

# set backtrace limit to not have infinite backtrace loops
set backtrace limit 32

# detect unhandled exceptions, hard faults and panics
# break DefaultHandler
break HardFault
break rust_begin_unwind
# # run the next few lines so the panic message is printed immediately
# # the number needs to be adjusted for your panic handler
# commands $bpnum
# next 4
# end

# *try* to stop at the user entry point (it might be gone due to inlining)
break main

# enable semihosting
monitor arm semihosting enable

# load the program
load

# start the process but immediately halt the processor
stepi

# inside _erase_page, on call to commit_erase
# break src/flc.rs:212
# inside commit_erase
# break src/flc.rs:155 
# c
# c
# c
# break core::ptr::read_volatile<u32>
# finish
# finish
