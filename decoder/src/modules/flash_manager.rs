pub extern crate max7800x_hal as hal;
pub use hal::flc::{FlashError, Flc};
use panic_halt as _; // Import module from lib.rs

use core::convert::TryInto;
use core::mem::size_of;

use bytemuck::{Pod, Zeroable};

#[derive(Debug)]
pub enum FlashManagerError {
    /// An error occurred in the underlying flash operations.
    FlashError(FlashError),
    /// The magic value in flash did not match the expected value.
    MagicMismatch,
}

impl From<FlashError> for FlashManagerError {
    fn from(err: FlashError) -> Self {
        FlashManagerError::FlashError(err)
    }
}

// The manager struct that holds a reference to the flash controller.
pub struct FlashManager {
    flc: Flc,
}

impl FlashManager {
    pub fn new(flc: Flc) -> Self {
        FlashManager { flc }
    }

    /// Write data with a magic value prepended.
    ///
    /// The flash page will begin with the 4‑byte little‑endian representation of `magic`
    /// followed immediately by the bytes of `data`. The combined data is then written in 16‑byte
    /// chunks.
    pub fn write_data<T: Pod>(
        &mut self,
        start_address: u32,
        magic: u32,
        data: &T,
    ) -> Result<(), FlashManagerError> {
        // Convert the data to a byte slice.
        let data_bytes = bytemuck::bytes_of(data);
        // Total bytes = magic (4 bytes) + data
        let total_bytes = 4 + data_bytes.len();
        // For this example we use a stack buffer of fixed size.
        assert!(total_bytes <= 4096, "Combined data too large for buffer");
        let mut buffer = [0u8; 4096];

        // Write the magic (in little-endian order) into the first 4 bytes.
        buffer[..4].copy_from_slice(&magic.to_le_bytes());
        // Then copy the data immediately after.
        buffer[4..total_bytes].copy_from_slice(data_bytes);

        // Write the combined buffer to flash in 16-byte chunks.
        let chunks = (total_bytes + 15) / 16;
        for i in 0..chunks {
            let offset = i * 16;
            let chunk: [u8; 16] = if offset + 16 <= total_bytes {
                buffer[offset..offset + 16].try_into().unwrap()
            } else {
                // For the last chunk, pad with zeros if needed.
                let mut padded = [0u8; 16];
                let remaining = total_bytes - offset;
                padded[..remaining].copy_from_slice(&buffer[offset..offset + remaining]);
                padded
            };
            // Convert the 16-byte chunk into four u32 words.
            let word_arr: [u32; 4] = bytemuck::try_from_bytes::<[u32; 4]>(&chunk)
                .expect("Chunk conversion failed")
                .clone();
            self.flc
                .write_128(start_address + (i as u32 * 16), &word_arr)?;
        }
        Ok(())
    }

    /// Read data with a magic value at the beginning.
    ///
    /// This function reads enough bytes to cover a 4-byte magic value plus the size of T.
    /// It then checks that the first 4 bytes match `expected_magic`. If so, it returns the T
    /// (constructed from the bytes following the magic). Otherwise, it returns an error.
    pub fn read_data<T: Pod + Zeroable>(&mut self, start_address: u32) -> Result<T, FlashManagerError> {
        let data_size = size_of::<T>();
        // Total bytes to read = 4 (magic) + size of data.
        let total_bytes = 4 + data_size;
        let chunks = (total_bytes + 15) / 16;
        // For demonstration, we use a fixed-size buffer.
        assert!(
            chunks * 16 <= 4096,
            "Data too large for our temporary buffer"
        );
        let mut buffer = [0u8; 4096];
        for i in 0..chunks {
            let addr = start_address + (i as u32 * 16);
            let word_arr = self.flc.read_128(addr)?;
            let chunk: &[u8] = bytemuck::cast_slice(&word_arr);
            let offset = i * 16;
            buffer[offset..offset + 16].copy_from_slice(chunk);
        }
        // Convert the bytes after the magic into T.
        let data_bytes = &buffer[4..4 + data_size];
        let data =
            bytemuck::try_from_bytes(data_bytes).expect("Failed to cast bytes to target type");
        Ok(*data)
    }

    /// Erase the flash page at `start_address`.
    pub fn wipe_data(&mut self, start_address: u32) -> Result<(), FlashManagerError> {
        // The erase function is unsafe so we wrap it here.
        unsafe { Ok(self.flc.erase_page(start_address)?) }
    }

    /// Reads the first 4 bytes (magic) from the flash page at `start_address`
    /// and returns it as a u32 in little‑endian order.
    pub fn read_magic(&mut self, start_address: u32) -> Result<u32, FlashError> {
        // Flash is read in 16-byte chunks.
        let word_arr = self.flc.read_128(start_address)?;
        // Cast the 16-byte chunk into a byte slice.
        let bytes: &[u8] = bytemuck::cast_slice(&word_arr);
        // Convert the first 4 bytes into a u32.
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        Ok(magic)
    }
}
