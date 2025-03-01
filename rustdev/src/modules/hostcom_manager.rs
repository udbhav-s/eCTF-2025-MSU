// Re-export the HAL and panic handler as needed.
pub extern crate max7800x_hal as hal;
use bytemuck::{Pod, Zeroable};
// embedded_io API allows usage of core macros like `write!`
use embedded_io::{Read, Write};

/// The magic byte used in all protocol messages.
pub const MSG_MAGIC: u8 = b'%';

/// Ref: https://rules.ectf.mitre.org/2025/specs/detailed_specs.html#decoder-interface
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

/// Message header for protocol packets.
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MessageHeader {
    pub magic: u8,
    pub opcode: u8,
    pub length: u16,
}

/// Channel information used in list messages.
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChannelInfo {
    pub channel_id: u32,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

impl ChannelInfo {
    /// Checks if the `ChannelInfo` instance is empty (all bytes are `0xFF`, the erased state).
    pub fn is_empty(&self) -> bool {
        let bytes: &[u8] = bytemuck::bytes_of(self);
        bytes.iter().all(|&b| b == 0xFF)
    }
}

pub fn read_ack<U: Read>(console: &mut U) -> Result<(), ()> {
    let mut buf = [0u8; 4];
    console.read_exact(&mut buf).map_err(|_| ())?;

    if buf[0] != MSG_MAGIC || buf[1] != MsgType::Ack as u8 {
        return Err(());
    }

    // TODO: Add a check for maximum packet length allowed in header based on our protocol

    Ok(())
}

pub fn write_ack<U: Write>(console: &mut U) -> Result<(), ()> {
    console.write_all(b"%A\x00\x00").map_err(|_| ())
}

pub fn read_header<U: Read>(console: &mut U) -> Result<MessageHeader, ()> {
    let mut hdr = MessageHeader::zeroed();

    while console
        .read_exact(core::slice::from_mut(&mut hdr.magic))
        .is_ok()
    {
        if hdr.magic == MSG_MAGIC {
            break;
        }
    }

    console
        .read_exact(core::slice::from_mut(&mut hdr.opcode))
        .map_err(|_| ())?;
    console
        .read_exact(&mut hdr.length.to_le_bytes())
        .map_err(|_| ())?;

    Ok(hdr)
}

pub fn write_debug<U: Write + Read>(console: &mut U, msg: &str) -> Result<(), ()> {
    let bytes = msg.as_bytes();

    // Send debug message header
    let hdr = MessageHeader {
        magic: MSG_MAGIC,
        opcode: MsgType::Debug as u8,
        length: bytes.len() as u16,
    };
    console
        .write_all(bytemuck::bytes_of(&hdr))
        .map_err(|_| ())?;

    // Debug messages are not sent an ACK, so we don't send them in chunks
    // Send entire message at once
    console.write_all(bytes).map_err(|_| ())?;

    Ok(())
}

pub fn write_channel<U: Write>(console: &mut U, channel: &ChannelInfo) -> Result<(), ()> {
    console
        .write_all(bytemuck::bytes_of(channel))
        .map_err(|_| ())
}

pub fn write_list<U: Write + Read>(console: &mut U, channels: &[ChannelInfo]) -> Result<(), ()> {
    let num_channels = channels.len() as u32;
    let channel_info_size = core::mem::size_of::<ChannelInfo>();
    let length = (size_of::<u32>() + channels.len() * channel_info_size) as u16;

    let hdr = MessageHeader {
        magic: MSG_MAGIC,
        opcode: MsgType::List as u8,
        length,
    };

    console
        .write_all(bytemuck::bytes_of(&hdr))
        .map_err(|_| ())?;

    if read_ack(console).is_ok() {
        console.write_all(&num_channels.to_le_bytes()).ok();
        for ch in channels {
            write_channel(console, ch).map_err(|_| ())?;
        }
    }

    Ok(())
}
