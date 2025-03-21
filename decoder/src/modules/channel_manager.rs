use crate::modules::flash_manager::{FlashManager, FlashManagerError};
use crate::modules::hostcom_manager::{ChannelInfo, MessageBody, MessageHeader};
use bytemuck::{Pod, Zeroable};
use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::{Signature, Verifier};
use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use md5::{Digest, Md5};
use crate::{HOST_KEY_PUB, DECODER_ID, DECODER_KEY, CHANNEL_0_SUBSCRIPTION};

use super::hostcom_manager::{write_debug, UartHalOps};

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
    pub node_ext: u8,       // This will be 1 (left) or 2 (right) (node_num % 2)
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

// const PAGE_SIZE: u32 = 0x2000;
// const NUM_PAGES: usize = 8;
// const BASE_ADDRESS: u32 = 0x1006_0000;

// Todo: Add more error types to SubscriptionError and use it here
pub fn check_subscription_valid_and_store(
    hdr: &MessageHeader,
    body: MessageBody,
    flash_manager: &mut FlashManager
) -> Result<(), ()>  {
    let verifying_key = VerifyingKey::from_public_key_der(HOST_KEY_PUB).map_err(|_| {})?;

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

    let mut passwords_data: [u8; 128*25] = [0; 128*25];
    passwords_data.copy_from_slice(&message[36..36+(128*25)]);

    cipher.apply_keystream(&mut passwords_data);

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
    return save_subscription(flash_manager, channel_subscription).map_err(|_| ());
}

fn get_subscription_addr(
    flash_manager: &mut FlashManager,
    channel_id: u32
) -> Option<u32> {
    let mut page_addr: Option<u32> = None;
    
    for i in 0..8 {
        // TODO: move flash base to a constants file, as well as flash magic
        let addr = 0x1006_2000 + (i as u32 * 0x2000);
        match flash_manager.read_magic(addr) {
            // Magic present, the page is occupied
            Ok(0xABCD) => {
                // Read the ChannelInfo header for the subscription
                if let Ok(stored_sub) = flash_manager.read_data::<ChannelInfo>(addr) {
                    if stored_sub.channel_id == channel_id {
                        // Found a matching subscription
                        page_addr = Some(addr);
                        break;
                    }
                }
            },
            Ok(_) => {}
            Err(_) => {}
        }
    }

    return page_addr;
}

pub fn save_subscription(
    flash_manager: &mut FlashManager,
    subscription: ChannelSubscription,
) -> Result<(), SubscriptionError> {

    let channel_id = subscription.info.channel_id;

    let mut page_addr: Option<u32> = None;
    
    for i in 0..8 {
        // TODO: move flash base to a constants file, as well as flash magic
        let addr = 0x1006_2000 + (i as u32 * 0x2000);
        match flash_manager.read_magic(addr) {
            // Magic present, the page is occupied
            Ok(0xABCD) => {
                if let Ok(stored_sub) = flash_manager.read_data::<ChannelInfo>(addr) {
                    if stored_sub.channel_id == channel_id {
                        // Found a matching subscription, overwrite it
                        page_addr = Some(addr);
                        break;
                    }
                }
            },
            // Magic is not present, assume page unoccupied
            Ok(_) => {
                page_addr = Some(addr);
                break;
            }
            Err(_) => {}
        }
    }

    if let Some(addr) = page_addr {
        flash_manager
            .wipe_data(addr)?;
        flash_manager
            .write_data(addr, 0xABCD, &subscription)?;

        return Ok(());
    } else {
        // No empty page or matching channel was found, this should not happen
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

pub fn decode_frame<U: UartHalOps>(
    flash_manager: &mut FlashManager,
    frame: &ChannelFrame,
    console: &mut U,
) -> Result<[u8; 64], ()> {

    write_debug(console, "In decode_frame\n");
    
    let subscription: &ChannelSubscription = match frame.channel {
        0 => {
            write_debug(console, "Matched channel 0 subscription\n");
            &CHANNEL_0_SUBSCRIPTION
        }
        _ => {
            write_debug(console, "Finding subscription address\n");

            let sub_page_addr = match get_subscription_addr(flash_manager, frame.channel) {
                Some(addr) => addr,
                None => return Err(()),
            };

            write_debug(console, "Found subscription address\n");

            &flash_manager.read_data::<ChannelSubscription>(sub_page_addr).map_err(|_| {})?
        }
    };

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

    write_debug(console, "Created node path\n");

    // let path_idx: usize = 0;

    node_num = 1;
    let mut i = 0;
    while i < 64 {
        // Look for corresponding node in subscription package
        for sub_idx in 0..128 {
            let c = &subscription.passwords.contents[sub_idx];
            // Password is uninitialized, break
            if c.node_ext == 0 {
                break;
            }
            
            let c_node_num: u128 = (c.node_trunc * 2) as u128 + (c.node_ext - 1) as u128;
            if c_node_num == node_num {
                password_node = Some(*c);
                break;
            }
        }

        if password_node.is_some() {
            break;
        }

        // Go to next child according to branch path
        node_num = node_num * 2 + (path[i] - 1) as u128;
        i += 1;
    }

    write_debug(console, "Iterated through path and subscription nodes\n");

    if password_node.is_none() {
        return Err(());
    }

    let mut password_bytes: [u8; 16] = password_node.ok_or(())?.password;

    write_debug(console, "Hashing nodes\n");

    for branch in path.iter() {
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

    write_debug(console, "Done hashing, extending password\n");

    // Extend password to 32 bytes
    let mut extended_password: [u8; 32] = [0; 32];
    extended_password[..16].copy_from_slice(&password_bytes);
    let mut hasher = Md5::new();
    hasher.update(&password_bytes);
    extended_password[16..].copy_from_slice(&hasher.finalize());

    write_debug(console, "Decrypting frame\n");

    // Decrypt frame
    let mut cipher = ChaCha20::new(&extended_password.into(), &frame.nonce.into());

    let mut decrypted_frame: [u8; 64] = [0; 64];
    decrypted_frame.copy_from_slice(&frame.encrypted_content[0..64]);

    cipher.apply_keystream(&mut decrypted_frame);

    return Ok(decrypted_frame)
}