pub extern crate max7800x_hal as hal;
pub use hal::flc::{FlashError, Flc};
use panic_halt as _; // Import module from lib.rs

use core::convert::TryInto;
use core::mem::size_of;

use bytemuck::{Pod, Zeroable};

// The manager struct that holds a reference to the flash controller.
pub struct FlashManager<'a> {
    flc: &'a mut Flc,
}

impl<'a> FlashManager<'a> {
    pub fn new(flc: &'a mut Flc) -> Self {
        FlashManager { flc }
    }

    /// Write an arbitrary type `T` (which must be Pod) into flash starting at `start_address`.
    /// This function will break the data into 128-bit (16-byte) chunks and pad the final chunk if needed.
    pub fn write_data<T: Pod>(&mut self, start_address: u32, data: &T) -> Result<(), FlashError> {
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
    pub fn read_data<T: Pod + Zeroable>(&mut self, start_address: u32) -> Result<T, FlashError> {
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
    pub fn wipe_data(&mut self, start_address: u32) -> Result<(), FlashError> {
        // The erase function is unsafe so we wrap it here.
        unsafe { self.flc.erase_page(start_address) }
    }
}
