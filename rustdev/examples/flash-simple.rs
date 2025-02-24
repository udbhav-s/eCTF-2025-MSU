#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
pub use hal::entry;
pub use hal::flc::{FlashError, Flc};
pub use hal::gcr::clocks::{Clock, SystemClock};
pub use hal::pac;
use panic_halt as _;

use core::convert::TryInto;
use core::mem::size_of;
use embedded_io::Write;

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

// The manager struct that holds a reference to the flash controller.
pub struct SubscriptionManager<'a> {
    flc: &'a Flc,
}

impl<'a> SubscriptionManager<'a> {
    pub fn new(flc: &'a Flc) -> Self {
        SubscriptionManager { flc }
    }

    /// Write an arbitrary type `T` (which must be Pod) into flash starting at `start_address`.
    /// This function will break the data into 128-bit (16-byte) chunks and pad the final chunk if needed.
    pub fn write_data<T: Pod>(&self, start_address: u32, data: &T) -> Result<(), FlashError> {
        // Convert the data to a byte slice.
        let bytes = bytemuck::bytes_of(data);
        let total_bytes = bytes.len();
        // Compute how many 16-byte chunks are needed.
        let chunks = (total_bytes + 15) / 16;
        for i in 0..chunks {
            let offset = i * 16;
            // For each chunk, prepare a 16-byte array.
            let chunk: [u8; 16] = if offset + 16 <= total_bytes {
                // If we have a full 16 bytes, take them directly.
                bytes[offset..offset + 16].try_into().unwrap()
            } else {
                // Otherwise, copy the remaining bytes and pad with zeros.
                let mut padded = [0u8; 16];
                let remaining = total_bytes - offset;
                padded[..remaining].copy_from_slice(&bytes[offset..]);
                padded
            };
            // Convert the 16-byte chunk into four u32 words (using little-endian order).
            let word_arr: [u32; 4] = bytemuck::try_from_bytes::<[u32; 4]>(&chunk)
                .expect("Chunk conversion failed")
                .clone();
            self.flc
                .write_128(start_address + (i as u32 * 16), &word_arr)?;
        }
        Ok(())
    }

    /// Read data from flash starting at `start_address` into a type `T`.
    /// T must be Pod and Zeroable so we can safely create an instance from raw bytes.
    /// Note: The flash is read in 16-byte chunks, and any extra bytes (if T's size is not a multiple of 16)
    /// will be ignored.
    pub fn read_data<T: Pod + Zeroable>(&self, start_address: u32) -> Result<T, FlashError> {
        let total_bytes = size_of::<T>();
        let chunks = (total_bytes + 15) / 16;
        // Create a temporary buffer for the data.
        // For demonstration we assume T is not too large; adjust the maximum size as needed.
        let padded_size = chunks * 16;
        assert!(padded_size <= 256, "Type T is too large for this example");
        let mut buffer = [0u8; 256];
        for i in 0..chunks {
            let addr = start_address + (i as u32 * 16);
            let word_arr = self.flc.read_128(addr)?;
            let chunk: &[u8] = bytemuck::cast_slice(&word_arr);
            let offset = i * 16;
            buffer[offset..offset + 16].copy_from_slice(chunk);
        }
        // Reinterpret the first `total_bytes` bytes as type T.
        let data = bytemuck::try_from_bytes(&buffer[..total_bytes])
            .expect("Failed to cast bytes to target type");
        Ok(*data)
    }

    /// Erase the flash page at `start_address`.
    pub fn wipe_data(&self, start_address: u32) -> Result<(), FlashError> {
        // The erase function is unsafe so we wrap it here.
        unsafe { self.flc.erase_page(start_address) }
    }
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
    let flc = hal::flc::Flc::new(p.flc, clks.sys_clk);
    write!(console, "Flash controller initialized!\r\n").unwrap();

    delay.delay_ms(1000);

    let sub_manager = SubscriptionManager::new(&flc);

    let sub = ChannelInfo {
        channel_id: 1,
        start_timestamp: 100,
        end_timestamp: 400,
        key: 1,
    };

    // Erase flash and handle potential error
    if let Err(e) = sub_manager.wipe_data(0x1006_0000) {
        write!(console, "Error erasing flash: {:?}\r\n", e).unwrap();
    } else {
        write!(console, "Flash erased.\r\n").unwrap();
    }

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
