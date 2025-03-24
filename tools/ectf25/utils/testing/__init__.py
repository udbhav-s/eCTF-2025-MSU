import unittest
import json
import struct
import random
from typing import List, Tuple
from loguru import logger
from ectf25_design import ChannelKeyDerivation
from ectf25_design.encoder import Encoder
from Crypto.PublicKey import ECC
from Crypto.Random import get_random_bytes
from ectf25_design import Secrets
from ectf25_design.gen_subscription import gen_subscription
from ectf25.utils.decoder import DecoderIntf, DecoderError
# from ectf25.utils.flash_board import reset_board

def reset_decoder(erase_flash=False):
    reset_board("/home/udbhav/Documents/ectf-2025/eCTF-2025-MSU/rustdev/target/thumbv7em-none-eabihf/debug/eCTF_2025_MSU", 50000, erase_flash)

def get_secrets() -> Secrets:
    try:
        with open("global.secrets") as f:
            return json.loads(f.read())
    except FileNotFoundError:
        with open("../global.secrets") as f:
            return json.loads(f.read())
    
def decoder_sub_channels(secrets_bytes, decoder: DecoderIntf, channels=[1, 2, 3, 4]):
    logger.debug(f"Resetting decoder subscriptions")
    for ch in channels:
        sub: bytes = gen_subscription(secrets_bytes, 0xdeadbeef, 0, 2**64-1, ch)
        decoder.subscribe(sub)

class TestDecoder(unittest.TestCase):
    @classmethod
    def setUpClass(self):
        """Set up test environment with secrets and test parameters"""
        random.seed(0xdedbeef)

        self.secrets = get_secrets()

        self.host_key = ECC.import_key(bytes.fromhex(self.secrets["host_key_priv"]))

        secrets_bytes = json.dumps(self.secrets).encode()
        self.secrets_bytes = secrets_bytes
        self.encoder = Encoder(secrets_bytes)

        self.decoder = DecoderIntf("/dev/ttyACM0")

        logger.disable("ectf25.utils.decoder")

        # Reset the decoder and erase full flash
        # reset_decoder(True)

    # def test_decode_single(self):
    #     # reset_decoder(True)

    #     # decoder_sub_channels(self.secrets_bytes, self.decoder)

    #     # reset_decoder(False)

    #     logger.debug("Testing decode single frame")

    #     channel = 1
    #     frame = b"Test frame data"
    #     frame = frame + b"\x00"*(64 - len(frame))
    #     timestamp = 123556789

    #     # Encode the frame
    #     encoded_frame = self.encoder.encode(channel, frame, timestamp)

    #     # Verify the signature
    #     content = encoded_frame[:-64]  # Everything except last 64 bytes

    #     # Decrypt and verify frame contents
    #     header = content[:24]
    #     channel, timestamp, _nonce = struct.unpack("<IQ12s", header)

    #     # Derive the frame key
    #     # channel_root = bytes.fromhex(self.secrets["channels"][str(channel)])
    #     # deriv = ChannelKeyDerivation(root=channel_root, height=64)
    #     # frame_key = deriv.extend_key(deriv.get_frame_key(timestamp))

    #     # print(f"Frame key bytes: {deriv.get_frame_key(timestamp)}")

    #     # Decode frame
    #     decoded_frame = self.decoder.decode(encoded_frame)

    #     logger.debug(b"Got decoded frame: " + decoded_frame)

    # def test_decode_wrong_signature(self):
    #     logger.debug("Testing decoder rejects wrong signature in frame")

    #     reset_decoder(True)

    #     decoder_sub_channels(self.secrets_bytes, self.decoder)

    #     reset_decoder(False)

    #     channel = 1
    #     frame = b"Test frame data"
    #     frame = frame + b"\x00"*(64 - len(frame))
    #     timestamp = 1234

    #     # Encode the frame
    #     encoded_frame = self.encoder.encode(channel, frame, timestamp)

    #     fake_frame = encoded_frame[:-64] + get_random_bytes(64)

    #     # Decode frame
    #     with self.assertRaises(Exception):
    #         decoded_frame = self.decoder.decode(fake_frame)

    def test_decode_random(self):
        logger.debug("Testing random subscription ranges and frames")

        sub_ranges: List[Tuple[int, int]] = [(0, 0) for i in range(4)]
        last_timestamps: List[int] = [-1 for _ in range(4)]  # Initialize last timestamps for each channel

        sub_ranges[0] = (0, 2**64-1)
        # Create random subscriptions for channels 1-3
        for i in range(1, 4):
            start = random.randint(0, 2**64 - 2)
            end = random.randint(start, 2**64 - 1)

            sub_ranges[i] = (start, end)

        decoder = DecoderIntf("/dev/ttyACM0")

        logger.disable("ectf25.utils.decoder")

        # Generate and load subscriptions
        for i in range(4):
            sub_range = sub_ranges[i]
            try:
                logger.debug(f"Writing random subscription for channel {i}")
                sub: bytes = gen_subscription(self.secrets_bytes, 0xdeadbeef, sub_range[0], sub_range[1], i)
                decoder.subscribe(sub)
            except Exception as e:
                # Expect error for channel 0 subscription
                if i != 0:
                    raise e
                else:
                    logger.debug(f"Pass, channel 0 subscription failed")
        
        # Channel 4 will not decode
        sub_ranges.append((None, None))
        
        # Try random frame
        for _ in range(30):
            # Generate random frame timestamp and channel
            channel = random.randint(0, 4)

            if 1 <= channel <= 3 and random.random() < 0.5:
                start, end = sub_ranges[channel]
                timestamp = random.randint(start, end)
            else:
                timestamp = random.randint(0, 2**64 - 1)

            # Generate random frame data
            raw_frame_data = get_random_bytes(64)

            # Encode the frame
            logger.debug(f"Encoding frame for channel {channel} at timestamp {timestamp}")
            encoded_frame = self.encoder.encode(channel, raw_frame_data, timestamp)

            start, end = sub_ranges[channel]
            if channel < 4 and timestamp > last_timestamps[channel] and start <= timestamp and timestamp <= end:
                # Frame should decode and match 
                try:
                    decoded_frame = decoder.decode(encoded_frame)
                except Exception as e:
                    self.fail(f"Decode failed for valid frame with error: {e}")
                
                self.assertEqual(decoded_frame, raw_frame_data, "Decoded frame does not match raw frame data")

                # Update last timestamp for the channel
                last_timestamps[channel] = timestamp
            else:
                with self.assertRaises(DecoderError):
                    decoded_frame = decoder.decode(encoded_frame)

if __name__ == '__main__':
    tester = TestDecoder()
    tester.setUp()
    # tester.test_decode_wrong_signature()
    # tester.test_decode_single()
    tester.test_decode_random()