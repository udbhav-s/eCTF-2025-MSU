use crate::modules::flash_manager::{FlashManager, FlashManagerError};
use crate::modules::hostcom_manager::{ChannelInfo, MessageBody, MessageHeader};
use crate::modules::constants::{BASE_ADDRESS, MAX_SUBS};
use bytemuck::{Pod, Zeroable, bytes_of};
use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::{Signature, Verifier};
use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use md5::{Digest, Md5};
use crate::{HOST_KEY_PUB, DECODER_ID, DECODER_KEY, CHANNEL_0_SUBSCRIPTION};

use super::constants::PAGE_SIZE;

#[derive(Clone, Copy)]
pub struct ActiveChannel {
    pub channel_id: u32,
    pub last_frame: u64,
    pub received: bool,
}

pub type ActiveChannelsList = [Option<ActiveChannel>; 9];

#[derive(Debug)]
pub enum SubscriptionError {
    InvalidChannelId,
    NoPageFound,
    FlashManagerError(FlashManagerError),
}

impl From<FlashManagerError> for SubscriptionError {
    fn from(error: FlashManagerError) -> Self {
        SubscriptionError::FlashManagerError(error)
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChannelPassword {
    pub node_trunc: u64,    // Upper 64 bits of the node in the tree (node_num // 2)
    pub node_ext: u8,       // This will be 1 (left) or 2 (right) (node_num % 2 + 1)
    pub password: [u8; 16],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChannelPasswords {
    pub contents: [ChannelPassword; 128],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChannelSubscription {
    pub info: ChannelInfo,
    pub passwords: ChannelPasswords,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChannelFrame {
    pub channel: u32,
    pub timestamp: u64,
    pub nonce: [u8; 12],
    pub encrypted_content: [u8; 64],
    pub signature: [u8; 64],
}

struct SubscriptionPageIterator<'a> {
    page_num: usize,
    return_empty: bool,
    flash_manager: &'a mut FlashManager,
}

impl Iterator for SubscriptionPageIterator<'_>  {
    type Item = (u32, Option<ChannelInfo>);

    fn next(&mut self) -> Option<Self::Item> {
        let addr = BASE_ADDRESS + (self.page_num as u32 * PAGE_SIZE);

        if self.page_num >= MAX_SUBS {
            return None;
        }

        match self.flash_manager.read_magic(addr) {
            // Magic present, the page is occupied
            Ok(0xABCD) => {
                // Read the ChannelInfo header for the subscription
                if let Ok(channel) = self.flash_manager.read_data::<ChannelInfo>(addr) {
                    self.page_num += 1;

                    Some((addr, Some(channel)))
                } else {
                    None
                }
            },
            // Unoccupied page
            Ok(_) => {
                if self.return_empty {
                    return Some((addr, None));
                } else {
                    // Empty page reached means none of the subsequent pages should have a subscription
                    return None;
                }
            }
            Err(_) => { None }
        }
    }
}

fn channel_subscriptions(flash_manager: &mut FlashManager, return_empty: bool) -> SubscriptionPageIterator {
    SubscriptionPageIterator { page_num: 0, return_empty, flash_manager }
}

pub fn initialize_active_channels(
    active_channels: &mut ActiveChannelsList,
    flash_manager: &mut FlashManager
) {
    let mut idx: usize = 1;

    // Initialize emergency channel subscription
    active_channels[0] = Some(ActiveChannel { channel_id: 0, last_frame: 0, received: false });

    for (_, c) in channel_subscriptions(flash_manager, false) {
        if let Some(channel) = c {
            active_channels[idx] = Some(ActiveChannel {
                channel_id: channel.channel_id,
                last_frame: 0,
                received: false
            });

            idx += 1;
        }
    }
}

pub fn validate_channel_timestamp(frame: &ChannelFrame, active_channels: &mut ActiveChannelsList) -> bool {
    for channel_opt in active_channels.iter_mut() {
        if let Some(channel) = channel_opt.as_mut() {
            if channel.channel_id != frame.channel {
                continue
            }

            if !channel.received {
                channel.received = true;
                channel.last_frame = frame.timestamp;
                return true;
            }
            else if channel.received && frame.timestamp > channel.last_frame {
                channel.last_frame = frame.timestamp;
                return true;
            }
            else {
                return false;
            }
        }
    }

    false
}

pub fn check_subscription_valid_and_store(
    hdr: &MessageHeader,
    body: MessageBody,
    flash_manager: &mut FlashManager,
    active_channels: &mut ActiveChannelsList
) -> Result<(), ()>  {
    let verifying_key = VerifyingKey::from_public_key_der(HOST_KEY_PUB).map_err(|_| {})?;

    let header_len = 36;

    let msg_len = hdr.length as usize - 64;
    let message = &body.data[..msg_len];
    let signature = &body.data[msg_len..hdr.length as usize];
    
    let sig_result = Signature::from_slice(signature);

    if let Err(_) = sig_result {
        return Err(());
    }

    let sig = sig_result.unwrap();
    
    let result = verifying_key.verify(message, &sig);
    
    if result.is_err() {
        // write_debug(&mut console, "Signature verification failed\n");
        return Err(());
    } else {
        // write_debug(&mut console, "Signature verification succeeded!\n");
    }

    let decoder_id = u32::from_le_bytes(message[0..4].try_into().unwrap());
    let start_timestamp = u64::from_le_bytes(message[4..12].try_into().unwrap());
    let end_timestamp = u64::from_le_bytes(message[12..20].try_into().unwrap());
    let channel_id = u32::from_le_bytes(message[20..24].try_into().unwrap());
    // Parse the 12-byte nonce from bytes 24-36
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&message[24..36]);

    // Check decoder id is valid
    if decoder_id != DECODER_ID {
        return Err(());
    }

    // Check if channel is channel 0
    if channel_id == 0 {
        return Err(());
    }

    let mut cipher = ChaCha20::new(&DECODER_KEY.into(), &nonce.into());

    let msg_passwords = &message[header_len..msg_len];

    let mut passwords_data: [u8; 128*25] = [0; 128*25];
    passwords_data[..(msg_len-header_len)].copy_from_slice(&msg_passwords);

    cipher.apply_keystream(&mut passwords_data[0..(msg_len - header_len)]);

    // Parse the passwords into ChannelPasswords
    let passwords = bytemuck::from_bytes::<ChannelPasswords>(&passwords_data);

    let channel_info = ChannelInfo {
        channel_id,
        start_timestamp,
        end_timestamp
    };

    let channel_subscription = ChannelSubscription {
        info: channel_info,
        passwords: *passwords,
    };

    // Store the subscription
    return save_subscription(flash_manager, channel_subscription, active_channels).map_err(|_| ());
}

fn get_subscription_addr(
    flash_manager: &mut FlashManager,
    channel_id: u32
) -> Option<u32> {
    let mut page_addr: Option<u32> = None;

    for (addr, c) in channel_subscriptions(flash_manager, false) {
        if let Some(stored_sub) = c {
            if stored_sub.channel_id == channel_id {
                // Found a matching subscription
                page_addr = Some(addr);
                break;
            }
        }
    }

    return page_addr;
}

pub fn save_subscription(
    flash_manager: &mut FlashManager,
    subscription: ChannelSubscription,
    active_channels: &mut ActiveChannelsList,
) -> Result<(), SubscriptionError> {

    let channel_id = subscription.info.channel_id;

    let mut page_addr: Option<u32> = None;

    for (addr, c) in channel_subscriptions(flash_manager, true) {
        if let Some(stored_sub) = c {
            if stored_sub.channel_id == channel_id {
                // Found a matching subscription
                page_addr = Some(addr);
                break;
            }
        } else {
            // Found an unoccupied page
            page_addr = Some(addr);
            break;
        }
    }

    if let Some(addr) = page_addr {
        flash_manager
            .wipe_data(addr)?;
        flash_manager
            .write_data(addr, 0xABCD, &subscription)?;

        // Activate subscription
        for i in 0..active_channels.len() {
            let channel_opt = &mut active_channels[i];
            if let Some(channel) = channel_opt.as_mut() {
                // Do nothing if subscription exists (don't reset monotonic timestamp counter)
                if channel.id == channel_id {
                    break;
                }
            } else {
                // None of the existing channels match - create new entry
                active_channels[i] = Some(ActiveChannel {
                    channel_id,
                    received: false,
                    last_frame: 0,
                });
                break;
            }
        }

        return Ok(());
    } else {
        // No empty page or matching channel was found, max subscriptions reached
        return Err(SubscriptionError::NoPageFound);
    }
}

pub fn read_channel(
    flash_manager: &mut FlashManager,
    address: u32,
) -> Result<ChannelInfo, FlashManagerError> {
    match flash_manager.read_magic(address) {
        Ok(_) => Ok(flash_manager.read_data::<ChannelSubscription>(address)?.info),
        Err(e) => Err(FlashManagerError::FlashError(e)),
    }
}

pub fn decode_frame(
    flash_manager: &mut FlashManager,
    frame: &ChannelFrame,
    active_channels: &mut ActiveChannelsList,
) -> Result<[u8; 64], ()> {
    // Verify frame signature
    let verifying_key = VerifyingKey::from_public_key_der(HOST_KEY_PUB).map_err(|_| {})?;

    let message = &bytes_of(frame)[..core::mem::size_of::<ChannelFrame>() - 64];
    let signature = &frame.signature;
    
    let sig_result = Signature::from_slice(signature);

    if let Err(_) = sig_result {
        return Err(());
    }

    let sig = sig_result.unwrap();
    
    let result = verifying_key.verify(message, &sig);
    
    if result.is_err() {
        // write_debug(&mut console, "Signature verification failed\n");
        return Err(());
    } else {
        // write_debug(&mut console, "Signature verification succeeded!\n");
    }

    // Signature verified; let's decrypt the frame
    let subscription: &ChannelSubscription = match frame.channel {
        0 => {
            &CHANNEL_0_SUBSCRIPTION
        }
        _ => {
            let sub_page_addr = match get_subscription_addr(flash_manager, frame.channel) {
                Some(addr) => addr,
                None => return Err(()),
            };

            &flash_manager.read_data::<ChannelSubscription>(sub_page_addr).map_err(|_| {})?
        }
    };

    if !validate_channel_timestamp(frame, active_channels) {
        return Err(());
    }

    let mut node_num: u128 = (frame.timestamp as u128) + ((1 as u128) << 64);

    let mut path: [u8; 64] = [0; 64];
    let mut path_idx = 64;

    while node_num > 1 {
        let branch: u8 = (node_num % 2 + 1).try_into().unwrap();
        path[path_idx-1] = branch;
        path_idx -= 1;
        node_num = node_num / 2;
    }

    let mut password_node: Option<ChannelPassword> = None;

    node_num = 1;
    let mut i = 0;
    while i < 65 {
        // Look for corresponding node in subscription package
        for sub_idx in 0..128 {
            let c = &subscription.passwords.contents[sub_idx];
            // Password is uninitialized, break
            if c.node_ext == 0 {
                break;
            }
            
            let c_node_num: u128 = (c.node_trunc as u128)*2  + (c.node_ext - 1) as u128;
            if c_node_num == node_num {
                password_node = Some(*c);
                break;
            }
        }

        // Password found, or we have checked the last node
        if password_node.is_some() || i == 64 {
            break;
        }

        // Go to next child according to branch path
        node_num = node_num * 2 + (path[i] - 1) as u128;
        i += 1;
    }

    if password_node.is_none() {
        return Err(());
    }

    let mut password_bytes: [u8; 16] = password_node.ok_or(())?.password;

    for branch in path[i..].iter() {
        let mut hasher = Md5::new();

        let mut pass_in: [u8; 17] = [0; 17];
        pass_in[..16].copy_from_slice(&password_bytes);

        match branch {
            1 => {
                pass_in[16] = b'L';
            }
            2 => {
                pass_in[16] = b'R';
            }
            _ => return Err(())
        }

        hasher.update(&pass_in);
        password_bytes = hasher.finalize().into();
    }

    // Extend password to 32 bytes
    let mut extended_password: [u8; 32] = [0; 32];
    extended_password[..16].copy_from_slice(&password_bytes);
    let mut hasher = Md5::new();
    hasher.update(&password_bytes);
    extended_password[16..].copy_from_slice(&hasher.finalize());

    // Decrypt frame
    let mut cipher = ChaCha20::new(&extended_password.into(), &frame.nonce.into());

    let mut decrypted_frame: [u8; 64] = [0; 64];
    decrypted_frame.copy_from_slice(&frame.encrypted_content[0..64]);

    cipher.apply_keystream(&mut decrypted_frame);

    return Ok(decrypted_frame)
}