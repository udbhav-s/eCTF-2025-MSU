#![no_std]
#![no_main]

pub mod modules;

pub extern crate max7800x_hal as hal;

use bytemuck;
pub use hal::entry;
pub use hal::flc::{FlashError, Flc};
pub use hal::gcr::clocks::{Clock, SystemClock};
pub use hal::pac;
use modules::channel_manager::{save_subscription, SubscriptionError};
use modules::flash_manager::FlashManager;
use modules::hostcom_manager::{
    read_ack, read_body, read_header, write_ack, write_debug, write_error, write_list,
    MessageHeader, MsgType, MSG_MAGIC,
};
use panic_halt as _; // Import panic handler

#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals.
    let p = pac::Peripherals::take().unwrap();
    // let core = pac::CorePeripherals::take().expect("Failed to take core peripherals");

    // Initialize system peripherals and clocks.
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk.set_source(&mut gcr.reg, &ipo).freeze();

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral.
    // let rate = clks.sys_clk.frequency;
    // let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Initialize and split the GPIO0 peripheral into pins.
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings.
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let mut console = hal::uart::UartPeripheral::uart0(p.uart0, &mut gcr.reg, rx_pin, tx_pin)
        .baud(115200)
        .clock_pclk(&clks.pclk)
        .parity(hal::uart::ParityBit::None)
        .build();

    // Initialize the flash controller.
    let flc = hal::flc::Flc::new(p.flc, clks.sys_clk);
    // Use the HAL's blocking write_byte for text output.
    for &b in b"Flash controller initialized!\r\n" {
        console.write_byte(b);
    }
    // delay.delay_ms(1000);

    let mut flash_manager = FlashManager::new(flc);

    loop {
        // Read the header using our new low-overhead function.
        let hdr = read_header(&mut console);
        match hdr.opcode {
            x if x == MsgType::List as u8 => {
                let _ = write_ack(&mut console);
                write_debug(&mut console, "List section in rust\n");
                let _ = write_list(&mut console, &mut flash_manager);
            }
            x if x == MsgType::Subscribe as u8 => {
                let _ = write_ack(&mut console);
                let body = read_body(&mut console, hdr.length);
                let result = save_subscription(&mut flash_manager, body);

                // Prepare a subscribe response header.
                let resp_hdr = MessageHeader {
                    magic: MSG_MAGIC,
                    opcode: MsgType::Subscribe as u8,
                    length: 0,
                };

                if let Err(SubscriptionError::InvalidChannelId) = result {
                    let _ = write_error(&mut console);
                } else {
                    // Write the response header byte-by-byte.
                    for &b in bytemuck::bytes_of(&resp_hdr) {
                        console.write_byte(b);
                    }
                    let _ = read_ack(&mut console);
                }
            }
            x if x == MsgType::Decode as u8 => {
                let _ = write_ack(&mut console);
                let body = read_body(&mut console, hdr.length);
                // Prepare a decode response header.
                let resp_hdr = MessageHeader {
                    magic: MSG_MAGIC,
                    opcode: MsgType::Decode as u8,
                    length: hdr.length - 12,
                };
                for &b in bytemuck::bytes_of(&resp_hdr) {
                    console.write_byte(b);
                }
                let _ = read_ack(&mut console);
                // Write the decoded frame (skip the first 12 bytes).
                for &b in &body.data[12..(hdr.length as usize)] {
                    console.write_byte(b);
                }
            }
            _ => {
                // Unsupported command: send a simple error message.
                for &b in b"We only support the List command right now!\n" {
                    console.write_byte(b);
                }
            }
        }
    }
}
