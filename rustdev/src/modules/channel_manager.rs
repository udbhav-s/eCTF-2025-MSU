use crate::modules::flash_manager::{FlashManager, FlashManagerError};
use crate::modules::hostcom_manager::{ChannelInfo, MessageBody, MessageHeader};
use bytemuck::{Pod, Zeroable};
use ed25519_dalek::pkcs8::DecodePublicKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::{Signature, Verifier};
use crate::{HOST_KEY_PUB, DECODER_ID};

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

    // TODO: Decrypt channel passwords using the decoder key and nonce

    // Parse the passwords into ChannelPasswords
    let passwords = bytemuck::from_bytes::<ChannelPasswords>(&message[36..36+(128*25)]);

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
