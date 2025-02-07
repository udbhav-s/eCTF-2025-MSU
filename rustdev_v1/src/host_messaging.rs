use core::convert::TryFrom;
use max7800x_hal::uart::BuiltUartPeripheral;
use max7800x_hal::pac::uart0::RegisterBlock;
use core::ops::Deref;

pub const MSG_MAGIC: u8 = b'%';

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    Decode = b'D',
    Subscribe = b'S',
    List = b'L',
    Ack = b'A',
    Debug = b'G',
    Error = b'E',
}

impl TryFrom<u8> for MessageType {
    type Error = ();
    
    fn try_from(value: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match value {
            b'D' => Ok(MessageType::Decode),
            b'S' => Ok(MessageType::Subscribe),
            b'L' => Ok(MessageType::List),
            b'A' => Ok(MessageType::Ack),
            b'G' => Ok(MessageType::Debug),
            b'E' => Ok(MessageType::Error),
            _ => Err(()),
        }
    }
}

#[repr(C, packed)]
pub struct MessageHeader {
    pub magic: u8,
    pub cmd: MessageType,
    pub len: u16,
}

pub struct HostMessaging<UART, RX, TX, CTS, RTS> 
where
    UART: Deref<Target = RegisterBlock>
{
    uart: BuiltUartPeripheral<UART, RX, TX, CTS, RTS>,
}

impl<UART, RX, TX, CTS, RTS> HostMessaging<UART, RX, TX, CTS, RTS>
where
    UART: Deref<Target = RegisterBlock>
{
    pub fn new(uart: BuiltUartPeripheral<UART, RX, TX, CTS, RTS>) -> Self {
        Self { uart }
    }

    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<(), ()> {
        for (i, byte) in buf.iter_mut().enumerate() {
            if i % 256 == 0 && i != 0 {
                self.write_ack()?;
            }
            *byte = self.uart.read_byte();
        }
        Ok(())
    }

    pub fn read_header(&mut self) -> Result<MessageHeader, ()> {
        let mut magic = self.uart.read_byte();
        
        while magic != MSG_MAGIC {
            magic = self.uart.read_byte();
        }

        let cmd_byte = self.uart.read_byte();
        let cmd = MessageType::try_from(cmd_byte).map_err(|_| ())?;

        let mut len_bytes = [0u8; 2];
        self.read_bytes(&mut len_bytes)?;
        let len = u16::from_le_bytes(len_bytes);

        Ok(MessageHeader {
            magic,
            cmd,
            len,
        })
    }

    pub fn write_bytes(&mut self, buf: &[u8], should_ack: bool) -> Result<(), ()> {
        for (i, &byte) in buf.iter().enumerate() {
            if i % 256 == 0 && i != 0 && should_ack {
                if self.read_ack()? != MessageType::Ack {
                    return Err(());
                }
            }
            self.uart.write_byte(byte);
        }
        Ok(())
    }

    pub fn write_packet(&mut self, msg_type: MessageType, buf: Option<&[u8]>) -> Result<(), ()> {
        let len = buf.map(|b| b.len() as u16).unwrap_or(0);
        let header = MessageHeader {
            magic: MSG_MAGIC,
            cmd: msg_type,
            len,
        };

        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<MessageHeader>(),
            )
        };
        self.write_bytes(header_bytes, false)?;

        if msg_type != MessageType::Debug && msg_type != MessageType::Ack {
            if self.read_ack()? != MessageType::Ack {
                return Err(());
            }
        }

        if let Some(data) = buf {
            self.write_bytes(data, msg_type != MessageType::Debug)?;
            
            if msg_type != MessageType::Debug && msg_type != MessageType::Ack {
                if self.read_ack()? != MessageType::Ack {
                    return Err(());
                }
            }
        }

        Ok(())
    }

    pub fn read_ack(&mut self) -> Result<MessageType, ()> {
        let header = self.read_header()?;
        if header.cmd == MessageType::Ack {
            Ok(MessageType::Ack)
        } else {
            Err(())
        }
    }

    pub fn write_ack(&mut self) -> Result<(), ()> {
        self.write_packet(MessageType::Ack, None)
    }
}