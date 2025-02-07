#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
pub use hal::pac;
pub use hal::entry;
use panic_halt as _;

use embedded_io::Write;

#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().expect("Failed to take core peripherals");
    // Initialize system peripherals and clocks
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk.set_source(&mut gcr.reg, &ipo).freeze();
     // Initialize a delay timer using the ARM SYST (SysTick) peripheral
     let rate = clks.sys_clk.frequency;
     let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let mut console = hal::uart::UartPeripheral::uart0(
        p.uart0,
        &mut gcr.reg,
        rx_pin,
        tx_pin
    )
        .baud(115200)
        .clock_pclk(&clks.pclk)
        .parity(hal::uart::ParityBit::None)
        .build();

    // Initialize the flash controller
    let flc = hal::flc::Flc::new(p.flc, clks.sys_clk);
    write!(console, "Flash controller initialized!\r\n").unwrap();

    delay.delay_ms(1000);

    // Erase page
    let target_address = 0x1006_0000;
    let target_page_num = flc.get_page_number(target_address).unwrap();
    let result = unsafe { flc.erase_page(target_address) };
    match result {
        Ok(_) => write!(console, "Page {} erased\r\n", target_page_num).unwrap(),
        Err(err) => write!(console, "ERROR! Could not erase page {}: {:?}", target_page_num, err).unwrap(),
    };
    // Read the value at address 0x1006_0004
    let target_address = 0x1006_0004;
    let result = flc.read_32(target_address);
    let data: u32 = match result {
        Ok(data) => data,
        Err(err) => {
            write!(console, "ERROR! Could not read data at 0x{:08X}: {:?}\r\n", target_address, err).unwrap();
            0
        }
    };
    // Should be 0xFFFF_FFFF since flash defaults to all 1's
    let expected = 0xFFFF_FFFF;
    write!(console, "0x{:08X}: 0x{:08X}\r\n", target_address, data).unwrap();
    assert_eq!(data, expected, "ERROR! Data at 0x{:08X} is not 0x{:08X}", target_address, expected);

    // Write a 32-bit value to address 0x1006_0004
    let target_address = 0x1006_0004;
    let desired_data = 0x7856_3412;
    let result = flc.write_32(target_address, desired_data);
    match result {
        Ok(_) => write!(console, "32-bit data written\r\n").unwrap(),
        Err(err) => write!(console, "ERROR! Write error: {:?}", err).unwrap(),
    };
    // Read the data back from flash memory
    let data: u32 = flc.read_32(target_address).unwrap();
    write!(console, "0x{:08X}: 0x{:08X}\r\n", target_address, data).unwrap();
    assert_eq!(data, desired_data, "ERROR! Data at 0x{:08X} is not 0x{:08X}", target_address, desired_data);

    // Test for NeedsErase error
    let address = 0x1006_0000;
    // We set 0x1006_0004 to 0x7856_3412 earlier - here we set it to 0xFFFF_FFFF
    // This is not valid! We can't turn 0 bits into 1 bits without erasing the page
    let bad_data = [0xDEADBEEF, 0xFFFFFFFF, 0xCAFEBABE, 0x00C0FFEE];
    let result = flc.write_128(address, &bad_data);
    assert_eq!(result, Err(hal::flc::FlashError::NeedsErase), "ERROR! Write should have returned NeedsErase error");

    // Let's erase the page and try again
    let target_address = 0x1006_0000;
    let target_page_num = flc.get_page_number(target_address).unwrap();
    let result = unsafe { flc.erase_page(target_address) };
    match result {
        Ok(_) => write!(console, "Page {} erased\r\n", target_page_num).unwrap(),
        Err(err) => write!(console, "ERROR! Could not erase page {}: {:?}", target_page_num, err).unwrap(),
    };
    // Now try writing the data again
    let result = flc.write_128(address, &bad_data);
    match result {
        Ok(_) => write!(console, "128-bit data written\r\n").unwrap(),
        Err(err) => write!(console, "ERROR! Write error: {:?}\r\n", err).unwrap(),
    };
    // Read the data back from flash memory
    let returned_data = flc.read_128(address).unwrap();
    write!(console, "0x{:08X}: 0x{:08X} 0x{:08X} 0x{:08X} 0x{:08X}\r\n",
        target_address, returned_data[0], returned_data[1], returned_data[2], returned_data[3]).unwrap();
    assert_eq!(returned_data, bad_data, "ERROR! Data at 0x{:08X} is not the same as what was written", target_address);

    write!(console, "SUCCESS! Flash tests passed!\r\n").unwrap();

    loop {
        cortex_m::asm::nop();
    }
}
