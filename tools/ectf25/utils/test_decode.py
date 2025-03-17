import json
import struct
import random
from ectf25_design import ChannelKeyDerivation
from ectf25_design.encoder import Encoder
from Crypto.PublicKey import ECC
from Crypto.Signature import eddsa
from Crypto.Cipher import ChaCha20
from ectf25_design import Secrets
from ectf25.utils.decoder import DecoderIntf

def get_secrets() -> Secrets:
    with open("../../../global.secrets") as f:
        return json.loads(f.read())

class DecoderTester():
    def setUp(self):
        """Set up test environment with secrets and test parameters"""
        random.seed(0xdedbeef)

        self.secrets = get_secrets()

        self.host_key = ECC.import_key(bytes.fromhex(self.secrets["host_key_priv"]))

        secrets_bytes = json.dumps(self.secrets).encode()
        self.encoder = Encoder(secrets_bytes)

    def test_decode(self):
        """Test the encode function of the Encoder class"""
        channel = 1
        frame = b"Test frame data"
        frame = frame + b"\x00"*(64 - len(frame))
        timestamp = 1234567890

        # Encode the frame
        print("Encoding the frame")
        encoded_frame = self.encoder.encode(channel, frame, timestamp)

        # Check that the encoded frame has the expected length
        # 16 bytes for header (4 for channel, 8 for timestamp, 12 for nonce)
        # + length of encrypted frame + 64 bytes for signature
        expected_length = 4 + 8 + 12 + len(frame) + 64
        # self.assertEqual(len(encoded_frame), expected_length)

        # Verify the signature
        content = encoded_frame[:-64]  # Everything except last 64 bytes
        signature = encoded_frame[-64:]  # Last 64 bytes

        # Create verifier using host's public key
        public_key = self.host_key.public_key()
        verifier = eddsa.new(public_key, 'rfc8032')

        # Verify signature is valid
        verifier.verify(content, signature)

        # Decrypt and verify frame contents
        header = content[:24]
        encrypted_frame_data = content[24:]
        channel, timestamp, nonce = struct.unpack("<IQ12s", header)

        # Derive the frame key
        channel_root = bytes.fromhex(self.secrets["channels"][str(channel)])
        deriv = ChannelKeyDerivation(root=channel_root, height=64)
        frame_key = deriv.extend_key(deriv.get_frame_key(timestamp))

        # Decrypt the frame data
        cipher = ChaCha20.new(key=frame_key, nonce=nonce)
        decrypted_frame = cipher.decrypt(encrypted_frame_data)


if __name__ == '__main__':
    tester = DecoderTester()
    tester.setUp()
    tester.test_decode()