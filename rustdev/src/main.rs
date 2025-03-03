#![no_std]
#![no_main]

pub mod modules;

pub extern crate max7800x_hal as hal;

use embedded_io::Write;
pub use hal::entry;
pub use hal::flc::{FlashError, Flc};
pub use hal::gcr::clocks::{Clock, SystemClock};
pub use hal::pac;
use modules::channel_manager::{read_all_channels, save_subscription};
use modules::flash_manager::FlashManager;
use modules::hostcom_manager::{
    read_body, read_header, write_ack, write_debug, write_list, MsgType,
};
use panic_halt as _; // Import module from lib.rs

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

    let mut flash_manager = FlashManager::new(flc);

    loop {
        match read_header(&mut console) {
            Ok(hdr) => {
                if hdr.opcode == MsgType::List as u8 {
                    write_ack(&mut console).unwrap();
                    write_debug(&mut console, "List section in rust\n").unwrap();
                    match read_all_channels(&mut flash_manager, 0x1006_0000) {
                        Ok(channels) => {
                            write_list(&mut console, &channels).unwrap();
                        }
                        Err(_) => {}
                    }
                } else if hdr.opcode == MsgType::Subscribe as u8 {
                    write_ack(&mut console).unwrap();
                    match read_body(&mut console, hdr.length) {
                        Ok(body) => {
                            let _ = save_subscription(&mut flash_manager, body);
                        }
                        Err(_) => (),
                    };
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
