#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;

pub use hal::pac;
pub use hal::entry;
use panic_halt as _;
use bytemuck::{Pod, Zeroable};

// embedded_io API allows usage of core macros like `write!`
use embedded_io::{Read, Write};

const MSG_MAGIC: u8 = b'%'; 

// Ref: https://rules.ectf.mitre.org/2025/specs/detailed_specs.html#decoder-interface
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Decode = b'D',
    Subscribe = b'S',
    List = b'L',
    Ack = b'A',
    Debug = b'G',
    Error = b'E',
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MessageHeader {
    magic: u8,
    opcode: u8,
    length: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ChannelInfo {
    channel_id: u32,
    start_timestamp: u64,
    end_timestamp: u64,
}

fn read_ack<U: Read>(console: &mut U) -> Result<(), ()> {
    let mut buf = [0u8; 4];
    console.read_exact(&mut buf).map_err(|_| ())?;
    
    if buf[0] != MSG_MAGIC || buf[1] != MsgType::Ack as u8 {
        return Err(());
    }

    // TODO: Add a check for maximum packet length allowed in header based on our protocol

    Ok(())
}

fn write_ack<U: Write>(console: &mut U) -> Result<(), ()> {
    console.write_all(b"%A\x00\x00").map_err(|_| ())
}

fn read_header<U: Read>(console: &mut U) -> Result<MessageHeader, ()> {
    let mut hdr = MessageHeader::zeroed();
    
    while console.read_exact(core::slice::from_mut(&mut hdr.magic)).is_ok() {
        if hdr.magic == MSG_MAGIC {
            break;
        }
    }

    console.read_exact(core::slice::from_mut(&mut hdr.opcode)).map_err(|_| ())?;
    console.read_exact(&mut hdr.length.to_le_bytes()).map_err(|_| ())?;

    Ok(hdr)
}

fn write_debug<U: Write + Read>(console: &mut U, msg: &str) -> Result<(), ()> {
    let bytes = msg.as_bytes();
    
    // Send debug message header
    let hdr = MessageHeader {
        magic: MSG_MAGIC,
        opcode: MsgType::Debug as u8,
        length: bytes.len() as u16
    };
    console.write_all(bytemuck::bytes_of(&hdr)).map_err(|_| ())?;

    // Debug messages are not sent an ACK, so we don't send them in chunks
    // Send entire message at once
    console.write_all(bytes).map_err(|_| ())?;

    Ok(())
}

fn write_channel<U: Write>(console: &mut U, channel: &ChannelInfo) -> Result<(), ()> {
    console.write_all(bytemuck::bytes_of(channel)).map_err(|_| ())
}

fn write_list<U: Write + Read>(console: &mut U) -> Result<(), ()> {
    let channels: [ChannelInfo; 2] = [
        ChannelInfo { channel_id: 1, start_timestamp: 100, end_timestamp: 23230000 },
        ChannelInfo { channel_id: 2, start_timestamp: 500, end_timestamp: 4200 },
    ];

    let num_channels = channels.len() as u32;
    let channel_info_size = core::mem::size_of::<ChannelInfo>();
    let length = (size_of::<u32>() + channels.len() * channel_info_size) as u16;

    let hdr = MessageHeader { magic: b'%', opcode: b'L', length };
    
    console.write_all(bytemuck::bytes_of(&hdr)).map_err(|_| ())?;
    
    if read_ack(console).is_ok() {
        console.write_all(&num_channels.to_le_bytes()).ok();
        for ch in &channels {
            write_channel(console, ch).map_err(|_| ())?;
        }
    }

    Ok(())
}


#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals
    let p = pac::Peripherals::take().unwrap();
    // Initialize system peripherals and clocks
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk.set_source(&mut gcr.reg, &ipo).freeze();
    // Initialize a delay timer using the ARM SYST (SysTick) peripheral

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let mut console: hal::uart::BuiltUartPeripheral<pac::Uart0, hal::gpio::Pin<0, 0, hal::gpio::Af1>, hal::gpio::Pin<0, 1, hal::gpio::Af1>, (), ()> = hal::uart::UartPeripheral::uart0(
        p.uart0,
        &mut gcr.reg,
        rx_pin,
        tx_pin
    )
        .baud(115200)
        .clock_pclk(&clks.pclk)
        .parity(hal::uart::ParityBit::None)
        .build();

    loop {
        match read_header(&mut console) {
            Ok(hdr) => {
                if hdr.opcode == MsgType::List as u8 {
                    write_ack(&mut console).unwrap();
                    write_debug(&mut console, "Hello from Rust!\n").unwrap();
                    write_list(&mut console).unwrap();
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
