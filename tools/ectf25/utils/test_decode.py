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
from ectf25.utils.decoder import DecoderIntf

def get_secrets() -> Secrets:
    with open("global.secrets") as f:
        return json.loads(f.read())

class DecoderTester():
    def setUp(self):
        """Set up test environment with secrets and test parameters"""
        random.seed(0xdedbeef)

        self.secrets = get_secrets()

        self.host_key = ECC.import_key(bytes.fromhex(self.secrets["host_key_priv"]))

        secrets_bytes = json.dumps(self.secrets).encode()
        self.secrets_bytes = secrets_bytes
        self.encoder = Encoder(secrets_bytes)

    def test_decode_single(self):
        """Test the encode function of the Encoder class"""
        channel = 1
        frame = b"Test frame data"
        frame = frame + b"\x00"*(64 - len(frame))
        timestamp = 1234

        # Encode the frame
        encoded_frame = self.encoder.encode(channel, frame, timestamp)

        # Verify the signature
        content = encoded_frame[:-64]  # Everything except last 64 bytes

        # Decrypt and verify frame contents
        header = content[:24]
        channel, timestamp, _nonce = struct.unpack("<IQ12s", header)

        # Derive the frame key
        channel_root = bytes.fromhex(self.secrets["channels"][str(channel)])
        deriv = ChannelKeyDerivation(root=channel_root, height=64)
        frame_key = deriv.extend_key(deriv.get_frame_key(timestamp))

        print(f"Frame key bytes: {deriv.get_frame_key(timestamp)}")

        # Decode frame
        decoder = DecoderIntf("/dev/ttyACM0")
        decoded_frame = decoder.decode(encoded_frame)

        print(decoded_frame)

    def test_decode_wrong_signature(self):
        """Test the encode function of the Encoder class"""
        channel = 1
        frame = b"Test frame data"
        frame = frame + b"\x00"*(64 - len(frame))
        timestamp = 1234

        # Encode the frame
        encoded_frame = self.encoder.encode(channel, frame, timestamp)

        fake_frame = encoded_frame[:-64] + get_random_bytes(64)

        # Decode frame
        decoder = DecoderIntf("/dev/ttyACM0")
        decoded_frame = decoder.decode(fake_frame)

        print(decoded_frame)

    def test_decode_random(self):
        sub_ranges: List[Tuple[int, int]] = [(0, 0) for i in range(4)]

        # Create random subscriptions for channels 0-3
        for i in range(4):
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
        
        # Try random frame
        for _ in range(1000):
            # Generate random frame timestamp and channel
            timestamp = random.randint(0, 2**64 - 1)
            channel = random.randint(0, 4)

            # Generate random frame data
            raw_frame_data = get_random_bytes(64)

            try:
                # Encode the frame
                logger.debug(f"Encoding frame for channel {channel} at timestamp {timestamp}")
                encoded_frame = self.encoder.encode(channel, raw_frame_data, timestamp)

                # Attempt to decode the frame
                decoded_frame = decoder.decode(encoded_frame)

                # Check if the frame is within the subscription range
                if channel < 4:
                    start, end = sub_ranges[channel]
                    if not (start <= timestamp <= end):
                        raise AssertionError("Frame decoded outside of subscription range")
                    # Check if decoded frame matches the raw frame data
                    if decoded_frame != raw_frame_data:
                        raise AssertionError("Decoded frame does not match raw frame data")
                else:
                    raise AssertionError("Channel 4 should not decode successfully")

            except Exception as e:
                # Expect error for frames outside subscription range or for channel 4
                if channel < 4:
                    start, end = sub_ranges[channel]
                    if start <= timestamp <= end:
                        raise e  # Unexpected error for valid frame
                elif channel == 4:
                    logger.debug(f"Pass, channel 4 frame failed as expected")


if __name__ == '__main__':
    tester = DecoderTester()
    tester.setUp()
    # tester.test_decode_wrong_signature()
    tester.test_decode_single()
    # tester.test_decode_random()