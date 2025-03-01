#![no_std]
#![no_main]

pub mod modules;

pub extern crate max7800x_hal as hal;
// use core::result;

use bytemuck::Zeroable;
use embedded_io::Write;
pub use hal::entry;
pub use hal::flc::{FlashError, Flc};
pub use hal::gcr::clocks::{Clock, SystemClock};
pub use hal::pac;
use modules::flash_manager::FlashManager;
use modules::hostcom_manager::{
    read_header, write_ack, write_debug, write_list, ChannelInfo, MsgType,
};
use panic_halt as _; // Import module from lib.rs

pub const PAGE_SIZE: u32 = 0x2000;
pub const NUM_PAGES: usize = 8;
pub const BASE_ADDRESS: u32 = 0x1006_2000;

fn read_all_channels(
    sub_manager: &mut FlashManager, // or whatever type provides `read_data`
    base_address: u32,
) -> Result<[ChannelInfo; NUM_PAGES], FlashError> {
    let mut channels: [ChannelInfo; NUM_PAGES] = [ChannelInfo::zeroed(); NUM_PAGES];

    for i in 0..NUM_PAGES {
        let addr = base_address + (i as u32 * PAGE_SIZE);
        let channel = sub_manager.read_data::<ChannelInfo>(addr)?;

        // Store actual data if valid; otherwise, leave the zeroed default
        if !channel.is_empty() {
            channels[i] = channel;
        }
    }

    Ok(channels)
}

#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals
    let p = pac::Peripherals::take().unwrap();
    // let core = pac::CorePeripherals::take().expect("Failed to take core peripherals");
    // Initialize system peripherals and clocks
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk.set_source(&mut gcr.reg, &ipo).freeze();
    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    // let rate = clks.sys_clk.frequency;
    // let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

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
    let flc = hal::flc::Flc::new(p.flc, clks.sys_clk);
    write!(console, "Flash controller initialized!\r\n").unwrap();
    // delay.delay_ms(1000);

    let mut sub_manager = FlashManager::new(flc);

    for i in 0..7 {
        let target_address = BASE_ADDRESS + (i as u32 * PAGE_SIZE);
        let target_page_num = i + 1;

        // Wipe the page
        let result = sub_manager.wipe_data(target_address);
        match result {
            Ok(_) => write!(console, "Page {} erased\r\n", target_page_num).unwrap(),
            Err(err) => write!(
                console,
                "ERROR! Could not erase page {}: {:?}\r\n",
                target_page_num, err
            )
            .unwrap(),
        };

        // Create a different ChannelInfo for each page (modify as needed)
        let sub = ChannelInfo {
            channel_id: i as u32 + 1,
            start_timestamp: 1000 * (i as u64),
            end_timestamp: 2000 * (i as u64),
        };

        // Write data and handle potential error
        if let Err(e) = sub_manager.write_data(target_address, &sub) {
            write!(
                console,
                "Error writing data to page {}: {:?}\r\n",
                target_page_num, e
            )
            .unwrap();
        } else {
            write!(console, "Data written to page {}.\r\n", target_page_num).unwrap();
        }
    }

    loop {
        match read_header(&mut console) {
            Ok(hdr) => {
                if hdr.opcode == MsgType::List as u8 {
                    write_ack(&mut console).unwrap();
                    write_debug(&mut console, "Hello from Rust!\n").unwrap();
                    match read_all_channels(&mut sub_manager, 0x1006_0000) {
                        Ok(channels) => {
                            write_list(&mut console, &channels).unwrap();
                        }
                        Err(_) => {}
                    }
                } else {
                    let _ = console.write_all(b"We only support the List command right now!\n");
                }
            }
            Err(_) => {
                let _ = console.write_all(b"There was an error!\n");
            }
        }
    }
}
