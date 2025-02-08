#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
use embedded_io::Read;
pub use hal::pac;
pub use hal::entry;
use hal::pac::Uart0;
use hal::uart::BuiltUartPeripheral;
use panic_halt as _;
use hal::gpio::{Af1,Pin};

// embedded_io API allows usage of core macros like `write!`
use embedded_io::Write;

// Ref: https://rules.ectf.mitre.org/2025/specs/detailed_specs.html#decoder-interface
struct MessageHeader {
    magic: u8,
    opcode: u8,
    length: u16,
}

fn read_ack(console: &BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>) -> Result<(), ()> {
    // Wait for header magic
    while console.read_byte() != b'%' {}
    let opcode = console.read_byte();

    // Make sure next header is for an ACK
    if opcode != b'A' {
        return Err(())
    }

    // Discard length bytes
    for _ in 0..2 {
        console.read_byte();
    }

    Ok(())
}

fn write_ack(console: &BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>) {
    let ack_msg = "%A\x00\x00";
    console.write_bytes(ack_msg.as_bytes());
}

fn read_header(console: &BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>) -> MessageHeader {
    let mut hdr = MessageHeader { magic: 0, opcode: 0, length: 0 };
    // Wait for header magic
    while console.read_byte() != b'%' {}
    hdr.magic = b'%';

    hdr.opcode = console.read_byte();

    // Read message length
    let mut length_bytes: [u8; 2] = [0; 2];
    console.read_bytes(&mut length_bytes);

    hdr.length = u16::from_le_bytes(length_bytes);

    hdr
}

fn write_list(console: &BuiltUartPeripheral<Uart0, Pin<0, 0, Af1>, Pin<0, 1, Af1>, (), ()>) {
    // Write message header
    let mut hdr = MessageHeader { magic: b'%', opcode: b'L', length: 0 };

    let body = b"Hello from Rust! This board is subscribed to 0 channels";

    hdr.length = u16::try_from(body.len()).unwrap();

    // Write bytes for header (TODO: do this by converting the struct to bytes)
    console.write_byte(hdr.magic);
    console.write_byte(hdr.opcode);
    console.write_bytes(&hdr.length.to_le_bytes());

    read_ack(&console).unwrap();

    console.write_bytes(body);
}


#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
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

    loop {
        // let hdr = read_header(&console);

        let mut test = [0; 4];
        console.read_exact(&mut test).unwrap();

        let hdr = MessageHeader { magic: 0, opcode: 0, length: 0 };

        console.write_bytes(b"Waddup udbhav\r\n");

        match hdr.opcode {
            b'L' => {
                write_ack(&console);
                
                write_list(&console);
            }
            _ => {
                write!(console, "We only support the List command right now!\n").unwrap();
                continue;
            }
        }

    }
}
