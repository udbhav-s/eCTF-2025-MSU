// Re-export the HAL as needed.
pub extern crate max7800x_hal as hal;
use crate::modules::channel_manager::read_channel;
use crate::modules::flash_manager::FlashManager;
use bytemuck::{Pod, Zeroable};

pub const MSG_MAGIC: u8 = b'%';

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
pub struct MessageHeader {
    pub magic: u8,
    pub opcode: u8,
    pub length: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MessageBody {
    pub data: [u8; 4096],
    pub length: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChannelInfo {
    pub channel_id: u32,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

/// A minimal trait that exposes the HAL’s blocking read_byte and write_byte methods.
/// (This is provided to decouple our functions from a specific UART type.)
pub trait UartHalOps {
    fn read_byte(&mut self) -> u8;
    fn write_byte(&mut self, byte: u8);
}

// Implement UartHalOps for the HAL’s BuiltUartPeripheral.
impl<UART, RX, TX, CTS, RTS> UartHalOps for hal::uart::BuiltUartPeripheral<UART, RX, TX, CTS, RTS>
where
    UART: core::ops::Deref<Target = crate::pac::uart0::RegisterBlock>,
{
    #[inline(always)]
    fn read_byte(&mut self) -> u8 {
        Self::read_byte(self)
    }
    #[inline(always)]
    fn write_byte(&mut self, byte: u8) {
        Self::write_byte(self, byte)
    }
}

/// Reads an ACK packet. Returns 0 on success, -1 on error.
#[inline(always)]
pub fn read_ack<U: UartHalOps>(console: &mut U) -> i32 {
    // Read header bytes: wait until we see the magic byte.
    let mut byte = console.read_byte();
    while byte != MSG_MAGIC {
        byte = console.read_byte();
    }
    let cmd = console.read_byte();
    if cmd != MsgType::Ack as u8 {
        return -1;
    }
    // Skip the 2-byte length.
    let _ = console.read_byte();
    let _ = console.read_byte();
    0
}

/// Writes an ACK packet.
#[inline(always)]
pub fn write_ack<U: UartHalOps>(console: &mut U) -> i32 {
    let ack = [MSG_MAGIC, MsgType::Ack as u8, 0, 0];
    for &b in &ack {
        console.write_byte(b);
    }
    0
}

/// Reads a message header from UART.
#[inline(always)]
pub fn read_header<U: UartHalOps>(console: &mut U) -> MessageHeader {
    let mut byte = console.read_byte();
    while byte != MSG_MAGIC {
        byte = console.read_byte();
    }
    let opcode = console.read_byte();
    let b0 = console.read_byte();
    let b1 = console.read_byte();
    MessageHeader {
        magic: MSG_MAGIC,
        opcode,
        length: u16::from_le_bytes([b0, b1]),
    }
}

/// Reads the message body in 256-byte chunks.
/// Acknowledges each chunk. Returns the filled MessageBody.
#[inline(always)]
pub fn read_body<U: UartHalOps>(console: &mut U, length: u16) -> MessageBody {
    let mut body = MessageBody::zeroed();
    let total = length as usize;
    let mut offset = 0;
    let mut chunk = [0u8; 256];
    while offset < total {
        let chunk_size = core::cmp::min(256, total - offset);
        for i in 0..chunk_size {
            chunk[i] = console.read_byte();
        }
        body.data[offset..offset + chunk_size].copy_from_slice(&chunk[..chunk_size]);
        offset += chunk_size;
        let _ = write_ack(console);
    }
    body.length = length;
    body
}

/// Writes a debug message. (Debug messages do not require ACKs.)
#[inline(always)]
pub fn write_debug<U: UartHalOps>(console: &mut U, msg: &str) {
    let bytes = msg.as_bytes();
    let header = MessageHeader {
        magic: MSG_MAGIC,
        opcode: MsgType::Debug as u8,
        length: bytes.len() as u16,
    };
    let hdr_bytes = bytemuck::bytes_of(&header);
    for &b in hdr_bytes {
        console.write_byte(b);
    }
    for &b in bytes {
        console.write_byte(b);
    }
}

// TODO: Check if i32 return type necessary
/// Writes a ChannelInfo structure.
#[inline(always)]
pub fn write_channel<U: UartHalOps>(console: &mut U, channel: &ChannelInfo) -> i32 {
    let bytes = bytemuck::bytes_of(channel);
    for &b in bytes {
        console.write_byte(b);
    }
    0
}

/// Writes a "list" message with channel information.
/// Mimics the C version by writing the header, waiting for an ACK,
/// sending a count and then each ChannelInfo.
#[inline(always)]
pub fn write_list<U: UartHalOps>(console: &mut U, flash_manager: &mut FlashManager) -> i32 {
    let mut count: u32 = 0;
    for i in 0..8 {
        let addr = 0x1006_2000 + (i as u32 * 0x2000);
        if flash_manager.read_magic(addr).unwrap_or(0) == 0xABCD {
            count += 1;
        }
    }
    let header = MessageHeader {
        magic: MSG_MAGIC,
        opcode: MsgType::List as u8,
        length: (core::mem::size_of::<u32>() + count as usize * core::mem::size_of::<ChannelInfo>())
            as u16,
    };
    let hdr_bytes = bytemuck::bytes_of(&header);
    for &b in hdr_bytes {
        console.write_byte(b);
    }
    if read_ack(console) != 0 {
        return -1;
    }
    // Write the channel count (u32 little-endian)
    for &b in &count.to_le_bytes() {
        console.write_byte(b);
    }
    for i in 0..count {
        let addr = 0x1006_2000 + (i as u32 * 0x2000);
        let ch = read_channel(flash_manager, addr).unwrap();
        if write_channel(console, &ch) != 0 {
            return -1;
        }
    }
    0
}

/// Writes an error message.
#[inline(always)]
pub fn write_error<U: UartHalOps>(console: &mut U) -> i32 {
    let err = [MSG_MAGIC, MsgType::Error as u8, 0, 0];
    for &b in &err {
        console.write_byte(b);
    }
    0
}
