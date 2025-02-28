import gdb

class FlashCtrlCommand(gdb.Command):
    """Decode the MAX78000 Flash Controller Control register (FLC_CTRL).

Usage: flashctrl [value_or_address]
If no argument is provided, the command reads a 32-bit value from address 0x40029008.
If the argument is a valid memory address, the command will read the 32-bit value from that address.
Otherwise, it assumes the argument is the register value.
    """
    def __init__(self):
        super(FlashCtrlCommand, self).__init__("flashctrl", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty):
        arg = arg.strip()
        default_addr = 0x40029008
        # If no argument is provided, default to using default_addr
        if not arg:
            print("No argument provided. Reading memory from default address 0x{:08X}".format(default_addr))
            num = default_addr
        else:
            try:
                num = int(arg, 0)
            except Exception:
                print("Error: Please provide a valid numeric value or address.")
                return

        # Attempt to read memory from the given number.
        try:
            inferior = gdb.selected_inferior()
            mem = inferior.read_memory(num, 4)
            reg_value = int.from_bytes(mem.tobytes(), byteorder='little')
            print("Reading memory from address 0x{:08X}: 0x{:08X}".format(num, reg_value))
        except Exception as e:
            # If reading memory fails, assume the number is a literal value.
            reg_value = num

        print("Decoding FLC_CTRL (0x{:08X}):".format(reg_value))
        
        # Decode each field
        unlock    = (reg_value >> 28) & 0xF
        lve       = (reg_value >> 25) & 0x1
        pend      = (reg_value >> 24) & 0x1
        erase_code= (reg_value >> 8)  & 0xFF
        pge       = (reg_value >> 2)  & 0x1
        me        = (reg_value >> 1)  & 0x1
        wr        = (reg_value >> 0)  & 0x1

        unlock_state = "Unlocked" if unlock == 2 else "Locked"
        print("  Flash Unlock Code: 0x{:X} ({})".format(unlock, unlock_state))
        print("  Low Voltage Enable (LVE): {}".format("Enabled" if lve == 1 else "Disabled"))
        print("  Flash Busy (PEND): {}".format("Busy" if pend == 1 else "Idle"))

        if erase_code == 0x00:
            erase_desc = "Erase disabled"
        elif erase_code == 0x55:
            erase_desc = "Page erase code set"
        elif erase_code == 0xAA:
            erase_desc = "Mass erase code set"
        else:
            erase_desc = "Unknown erase code (0x{:02X})".format(erase_code)
        print("  Erase Code: 0x{:02X} ({})".format(erase_code, erase_desc))
        print("  Page Erase (PGE): {}".format("In progress" if pge == 1 else "Not in progress"))
        print("  Mass Erase (ME): {}".format("In progress" if me == 1 else "Not in progress"))
        print("  Write (WR): {}".format("In progress" if wr == 1 else "Not in progress"))

FlashCtrlCommand()

