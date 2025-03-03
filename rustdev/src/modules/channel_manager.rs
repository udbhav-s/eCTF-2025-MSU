use crate::modules::flash_manager::{FlashError, FlashManager};
use crate::modules::hostcom_manager::{ChannelInfo, MessageBody};

use super::flash_manager::MyFlashError;

// const PAGE_SIZE: u32 = 0x2000;
// const NUM_PAGES: usize = 8;
// const BASE_ADDRESS: u32 = 0x1006_0000;

pub fn save_subscription(
    flash_manager: &mut FlashManager,
    subscription: MessageBody,
) -> Result<(), FlashError> {
    // Extract subscription fields from the MessageBody.
    // (Assumes the MessageBody's last bytes store: channel_id (4 bytes), end_timestamp (8 bytes),
    //  and start_timestamp (8 bytes) in that order.)
    let channel_id = u32::from_le_bytes(
        subscription.data[(subscription.length - 4) as usize..subscription.length as usize]
            .try_into()
            .unwrap(),
    );
    let end_timestamp = u64::from_le_bytes(
        subscription.data[(subscription.length - 12) as usize..(subscription.length - 4) as usize]
            .try_into()
            .unwrap(),
    );
    let start_timestamp = u64::from_le_bytes(
        subscription.data[(subscription.length - 20) as usize..(subscription.length - 12) as usize]
            .try_into()
            .unwrap(),
    );

    let new_sub = ChannelInfo {
        channel_id,
        start_timestamp,
        end_timestamp,
    };

    Ok(for i in 0..8 {
        let addr = 0x1006_0000 + (i as u32 * 0x2000);
        match flash_manager.read_magic(addr) {
            Ok(magic) => {
                if magic != 0xABCD {
                    // If magic doesn't match, consider this page empty.
                    flash_manager.wipe_data(addr)?;
                    flash_manager.write_data(addr, 0xABCD, &new_sub)?;
                    break;
                } else {
                    // Magic present, so read the stored ChannelInfo.
                    // Note: read_data returns a type T if the magic is correct.
                    if let Ok(stored_sub) = flash_manager.read_data::<ChannelInfo>(addr) {
                        if stored_sub.channel_id == channel_id {
                            // Found a matching subscription.
                            flash_manager.wipe_data(addr)?;
                            flash_manager.write_data(addr, 0xABCD, &new_sub)?;
                            break;
                        }
                    }
                }
            }
            Err(_) => {}
        }
    })
}

pub fn read_channel(
    flash_manager: &mut FlashManager,
    address: u32,
) -> Result<ChannelInfo, MyFlashError> {
    match flash_manager.read_magic(address) {
        Ok(_) => flash_manager.read_data::<ChannelInfo>(address),
        Err(e) => Err(MyFlashError::FlashError(e)),
    }
}
