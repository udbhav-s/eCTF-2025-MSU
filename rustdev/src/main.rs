#![no_std]
#![no_main]

pub mod modules;

pub extern crate max7800x_hal as hal;

use bytemuck;
use ed25519_dalek::pkcs8::DecodePrivateKey;
pub use hal::entry;
pub use hal::flc::{FlashError, Flc};
pub use hal::gcr::clocks::{Clock, SystemClock};
pub use hal::pac;
use md5::{Digest, Md5};
use ed25519_dalek::{Signature, Verifier, SigningKey};
// use ed25519_dalek::{Signature, Verifier, SigningKey, pkcs8::DecodePrivateKey};
use modules::channel_manager::{save_subscription, SubscriptionError};
use modules::flash_manager::FlashManager;
use modules::hostcom_manager::{
    read_ack, read_body, read_header, write_ack, write_debug, write_error, write_list,
    MessageHeader, MsgType, MSG_MAGIC,
};
use panic_halt as _; // Import panic handler

use embedded_io::Write;

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
                let key_der = b"\\x30\\x2e\\x02\\x01\\x00\\x30\\x05\\x06\\x03\\x2b\\x65\\x70\\x04\\x22\\x04\\x20\\xdf\\x05\\x18\\x15\\x4c\\xcc\\xae\\x9a\\xb4\\xf4\\x8b\\x5c\\xb4\\xc0\\xfb\\x59\\x87\\xec\\x5b\\x94\\x98\\x3a\\x9a\\x6c\\x12\\xd4\\x8b\\xc5\\xb1\\x19\\xcb\\x5b";
                let signing_key = SigningKey::from_pkcs8_der(key_der).expect("Invalid key DER!");
                let verifying_key = signing_key.verifying_key();

                let _ = write_ack(&mut console);
                let body = read_body(&mut console, hdr.length);

                let msg_len = hdr.length as usize - 64;
                let message = &body.data[..msg_len];
                let signature = &body.data[msg_len..hdr.length as usize];
                
                let sig = Signature::from_slice(signature)
                    .expect("Failed to parse signature");
                
                let result = verifying_key.verify(message, &sig);
                
                if result.is_err() {
                    write_debug(&mut console, "Signature verification failed\n");
                    let _ = write_error(&mut console);
                    continue;
                } else {
                    write_debug(&mut console, "Signature verification succeeded!\n");
                }

                let decoder_id = u32::from_le_bytes(message[0..4].try_into().unwrap());
                let start_ts = u64::from_le_bytes(message[4..12].try_into().unwrap());
                let end_ts = u64::from_le_bytes(message[12..20].try_into().unwrap());
                let channel_id = u32::from_le_bytes(message[20..24].try_into().unwrap());
                // Note: bytes 24..36 contain the encryption nonce

                // Debug prints for header fields
                write!(console, "Decoder ID: {}\r\n", decoder_id).unwrap();
                write!(console, "Start timestamp: {}\r\n", start_ts).unwrap();
                write!(console, "End timestamp: {}\r\n", end_ts).unwrap();
                write!(console, "Channel ID: {}\r\n", channel_id).unwrap();

                // let result = save_subscription(&mut flash_manager, body);

                // Prepare a subscribe response header.
                let resp_hdr = MessageHeader {
                    magic: MSG_MAGIC,
                    opcode: MsgType::Subscribe as u8,
                    length: 0,
                };

                // if let Err(SubscriptionError::InvalidChannelId) = result {
                //     let _ = write_error(&mut console);
                // } else {
                    // Write the response header byte-by-byte.
                    for &b in bytemuck::bytes_of(&resp_hdr) {
                        console.write_byte(b);
                    }
                    let _ = read_ack(&mut console);
                // }
            }
            x if x == MsgType::Decode as u8 => {
                let _ = write_ack(&mut console);
                let body = read_body(&mut console, hdr.length);

                // hashes
                let h = b"hello world";
                for _i in 0..64 {
                    let mut hasher = Md5::new();
                    hasher.update(h);
                    let _hash = hasher.finalize();
                }

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
