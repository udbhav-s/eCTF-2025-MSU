#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
use ectf_2025_msu::modules::flash_manager::FlashManager;
use embedded_io::Write;
pub use hal::entry;
pub use hal::flc::{FlashError, Flc};
pub use hal::gcr::clocks::{Clock, SystemClock};
pub use hal::pac;
use panic_halt as _; // Import module from lib.rs

use bytemuck::{Pod, Zeroable};

// Example: define a subscription record.
#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, packed)]
struct ChannelInfo {
    channel_id: u32,
    start_timestamp: u64,
    end_timestamp: u64,
    key: u16,
}

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
    let mut console = hal::uart::UartPeripheral::uart0(p.uart0, &mut gcr.reg, rx_pin, tx_pin)
        .baud(115200)
        .clock_pclk(&clks.pclk)
        .parity(hal::uart::ParityBit::None)
        .build();

    // Initialize the flash controller
    let mut flc = hal::flc::Flc::new(p.flc, clks.sys_clk);
    write!(console, "Flash controller initialized!\r\n").unwrap();

    delay.delay_ms(1000);

    let mut sub_manager = FlashManager::new(&mut flc);

    let sub = ChannelInfo {
        channel_id: 1,
        start_timestamp: 100,
        end_timestamp: 400,
        key: 1,
    };

    let target_address = 0x1006_0000;
    let result = sub_manager.wipe_data(target_address);
    match result {
        Ok(_) => write!(console, "Page {} erased\r\n", 1).unwrap(),
        Err(err) => write!(console, "ERROR! Could not erase page {}: {:?}", 1, err).unwrap(),
    };

    // Write data and handle potential error
    if let Err(e) = sub_manager.write_data(0x1006_0000, &sub) {
        write!(console, "Error writing data: {:?}\r\n", e).unwrap();
    } else {
        write!(console, "Data written.\r\n").unwrap();
    }

    let result = sub_manager.read_data::<ChannelInfo>(0x1006_0000);
    match result {
        Ok(read_sub) => write!(
            console,
            "Read subscription: channel_id: {}, start_timestamp: {}, end_timestamp: {}, key: {}\r\n",
            read_sub.channel_id as u32,
            read_sub.start_timestamp as u64,
            read_sub.end_timestamp as u64,
            read_sub.key as u16,
        )
        .unwrap(),
        Err(err) => write!(console, "ERROR! Could not read subscription: {:?}\r\n", err).unwrap(),
    };

    loop {
        cortex_m::asm::nop();
    }
}
