# From the design directory, run (with venv activated):
# `python -m unittest`

import unittest
import json
from ectf25_design import gen_secrets
from Crypto.PublicKey import ECC
from Crypto.Signature import eddsa
from Crypto.Hash import SHA512
from ectf25_design import Secrets
from typing import List

def get_secrets(channels = [1, 3, 4]) -> Secrets:
    return json.loads(gen_secrets(channels).decode())

class TestGenSecrets(unittest.TestCase):
    def test_valid_secret_keys(self):
        """Test that all secret keys are properly formatted"""
        secrets = get_secrets()
        
        # Test channel secrets can be decoded as bytes
        for channel, secret in secrets["channels"].items():
            try:
                bytes.fromhex(secret)
            except ValueError:
                self.fail(f"Channel {channel} secret is not valid hex")
                
        # Test decoder_dk can be decoded as bytes
        try:
            bytes.fromhex(secrets["decoder_dk"])
        except ValueError:
            self.fail("decoder_dk is not valid hex")
            
        # Test host_key can be imported as ECC key
        try:
            ECC.import_key(secrets["host_key"])
        except ValueError:
            self.fail("host_key is not a valid PEM-encoded ECC key")
    
    def test_channel_key_inclusion(self):
        """Test that secrets only contain keys for specified channels, and 0"""
        import random
        
        # Test with several random channel lists
        for _ in range(3):
            # Generate random channel list (any positive integer)
            num_channels = random.randint(1, 10)
            # Generate random channel numbers
            test_channels: List[int] = []
            for _ in range(num_channels):
                channel = random.randint(1, 2**31-1)
                test_channels.append(channel)

            secrets = get_secrets(test_channels)
            
            # Check that only specified channels are included, including 0
            original_channels = set([int(key) for key in secrets["channels"].keys()])
            original_channels.add(0)

            test_channels = set(test_channels)

            self.assertEqual(
                original_channels,
                test_channels,
                "Secrets contain channels that weren't specified"
            )

    def test_signing_with_host_key(self):
        """Test signing and verifying a message with the host key"""
        secrets = get_secrets()
        
        # Parse the PEM-encoded Ed25519 private key
        private_key = ECC.import_key(secrets["host_key"])
        
        # Create signer object
        signer = eddsa.new(private_key, 'rfc8032')
        
        # Create a test message and hash it
        test_message = b"Test message for signature verification"
        message_hash = SHA512.new(test_message)
        
        # Sign the hash
        signature = signer.sign(message_hash)
        
        # Get public key for verification
        public_key = private_key.public_key()
        
        # Create verifier
        verifier = eddsa.new(public_key, 'rfc8032')
        
        # Verify the signature
        verifier.verify(message_hash, signature)
        
        # Test that verification fails with wrong message
        wrong_message = SHA512.new(b"Wrong message")
        with self.assertRaises(ValueError):
            verifier.verify(wrong_message, signature)
            
        # Test that verification fails with wrong signature
        wrong_signature = signature[:-1] + bytes([signature[-1] ^ 1])
        with self.assertRaises(ValueError):
            verifier.verify(message_hash, wrong_signature)

if __name__ == '__main__':
    unittest.main()