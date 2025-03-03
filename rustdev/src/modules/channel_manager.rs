use crate::modules::flash_manager::{FlashError, FlashManager};
use crate::modules::hostcom_manager::{ChannelInfo, MessageBody};
use bytemuck::Zeroable;

const PAGE_SIZE: u32 = 0x2000;
const NUM_PAGES: usize = 8;
const BASE_ADDRESS: u32 = 0x1006_0000;

pub fn save_subscription(
    flash_manager: &mut FlashManager,
    subscription: MessageBody,
) -> Result<(), FlashError> {
    // TODO: This will have to get revamped

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

    let sub = ChannelInfo {
        channel_id,
        start_timestamp,
        end_timestamp,
    };

    let result = flash_manager.wipe_data(0x1006_0000);
    match result {
        Ok(_) => flash_manager.write_data(0x1006_0000, &sub),
        Err(e) => Err(e),
    }
}

pub fn read_all_channels(
    sub_manager: &mut FlashManager, // or whatever type provides `read_data`
    base_address: u32,
) -> Result<[ChannelInfo; NUM_PAGES], FlashError> {
    let mut channels: [ChannelInfo; NUM_PAGES] = [ChannelInfo::zeroed(); NUM_PAGES];

    for i in 0..NUM_PAGES {
        let addr = base_address + (i as u32 * PAGE_SIZE);
        let channel = sub_manager.read_data::<ChannelInfo>(addr)?;

        // Store actual data if valid; otherwise, leave the zeroed default
        if !channel.is_empty() {
            channels[i] = channel;
        }
    }

    Ok(channels)
}
