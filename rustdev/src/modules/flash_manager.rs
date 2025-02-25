pub extern crate max7800x_hal as hal;
pub use hal::flc::{FlashError, Flc};
use panic_halt as _;

use core::convert::TryInto;
use core::mem::size_of;

use bytemuck::{Pod, Zeroable};

/// The manager now holds a mutable reference to ensure exclusive access.
pub struct FlashManager<'a> {
    flc: &'a mut Flc,
}

impl<'a> FlashManager<'a> {
    pub fn new(flc: &'a mut Flc) -> Self {
        FlashManager { flc }
    }

    /// Write an arbitrary type `T` (which must be Pod) into flash.
    pub fn write_data<T: Pod>(&mut self, start_address: u32, data: &T) -> Result<(), FlashError> {
        let bytes = bytemuck::bytes_of(data);
        let total_bytes = bytes.len();
        let chunks = (total_bytes + 15) / 16;
        for i in 0..chunks {
            let offset = i * 16;
            let chunk: [u8; 16] = if offset + 16 <= total_bytes {
                bytes[offset..offset + 16].try_into().unwrap()
            } else {
                let mut padded = [0u8; 16];
                let remaining = total_bytes - offset;
                padded[..remaining].copy_from_slice(&bytes[offset..]);
                padded
            };
            let word_arr: [u32; 4] = bytemuck::try_from_bytes::<[u32; 4]>(&chunk)
                .expect("Chunk conversion failed")
                .clone();
            self.flc
                .write_128(start_address + (i as u32 * 16), &word_arr)?;
        }
        Ok(())
    }

    /// Read data from flash into a type `T`.
    pub fn read_data<T: Pod + Zeroable>(&mut self, start_address: u32) -> Result<T, FlashError> {
        let total_bytes = size_of::<T>();
        let chunks = (total_bytes + 15) / 16;
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
        let data = bytemuck::try_from_bytes(&buffer[..total_bytes])
            .expect("Failed to cast bytes to target type");
        Ok(*data)
    }

    /// Erase the flash page at `start_address`.
    pub fn wipe_data(&mut self, start_address: u32) -> Result<(), FlashError> {
        // Call the unsafe erase_page, ensuring exclusive access.
        unsafe { self.flc.erase_page(start_address) }
    }
}
