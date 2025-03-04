# From the design directory, run (with venv activated):
# `python -m unittest`

import unittest
import json
import random
from ectf25_design import gen_secrets, ChannelKeyDerivation, ChannelTreeNode
from Crypto.PublicKey import ECC
from Crypto.Signature import eddsa
from Crypto.Hash import SHA512
from ectf25_design import Secrets
from typing import List, Tuple

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
                channel = random.randint(1, 2**32-1)
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
    
class TestGenSubscription(unittest.TestCase):
    def test_get_node_cover(self):
        """Test that function to get a cover for a node is correct"""
        h = 64
        deriv = ChannelKeyDerivation(root=b"1234", height=h)

        end = 2**h - 1

        # Left subtrees at successive depths
        for i in range(0, 8):
            self.assertEqual(deriv.get_channel_node_cover(2**i), (0, 2**(h-i) - 1))
        
        # Right subtrees at successive depths
        for i in range(0, 8):
            span = 2**(h - i)
            self.assertEqual(deriv.get_channel_node_cover(2**(i+1) - 1), (end - span + 1, end))
        
        # Left subtree of right subtree
        span = 2**(h - 2)
        self.assertEqual(deriv.get_channel_node_cover(6), (span*2, end - span))
    
    def test_get_covering_nodes(self):
        """Test that cover of node numbers created by get_covering_nodes is correct"""
        random.seed(0xdeadbeef)

        h = 64
        deriv = ChannelKeyDerivation(root=b"1234", height=h)

        test_ranges: List[Tuple[int, int]] = [
            # Full range
            (0, 2**h - 1),
            # Left subtree at depth 2
            (0, 2**(h-2) - 1),
            # Last two frames (bottom right most subtree)
            (2**h - 2, 2**h - 1)
        ]

        for r in test_ranges:
            start, end = r
            cover = deriv.get_covering_nodes(start, end)
            self.assertEqual(deriv.get_channel_nodes_cover(cover), (start, end))

        password_counts = []
        
        # Test random ranges
        rounds = 1000
        for _ in range(rounds):
            start = random.randint(0, 2**h - 1)
            end = start + random.randint(0, 2**h - 1 - start)
            cover = deriv.get_covering_nodes(start, end)
            password_counts.append(len(cover))
            self.assertEqual(deriv.get_channel_nodes_cover(cover), (start, end))
        
        # print("Number of passwords needed to cover 1000 random frame ranges:")
        # print(password_counts)

    def test_generate_keys_from_node_cover(self):
        """Test that the keys created for a subscription have the correct cover and secrets"""
        random.seed(0xdeafbeef)

        h = 64
        deriv = ChannelKeyDerivation(root=b"1234", height=h)

        # Test that for full range, traversing to any leaf gives the corresponding key for that frame
        for _ in range(100):
            curr_key = deriv.root;
            node_num = 1
            steps = random.randint(0, h)
            # traversal = []
            for _ in range(steps):
                choice = random.randint(0, 1)
                # traversal.append(choice)
                if choice == 0:
                    curr_key = deriv.get_left_subkey(curr_key)
                    node_num = node_num * 2
                else:
                    curr_key = deriv.get_right_subkey(curr_key)
                    node_num = node_num * 2 + 1

            channel_key = deriv.get_key_for_node(node_num)
            self.assertEqual(channel_key, ChannelTreeNode(node_num, curr_key))

        # print(f"Test traversal for {node_num}:")
        # print(traversal)

    def test_get_frame_key_from_cover(self):
        """Test that get_frame_key_from_cover is able to derive keys for frames within its range
        """

        # Here we generate a random subscription range, and get the set of keys for it
        # Then we make sure it doesn't throw an error, and the key is the same as deriving it manually
        random.seed(0xdeadbeef)
        
        h = 4
        deriv = ChannelKeyDerivation(root=b"1234", height=h)
        
        # Test several random ranges
        for _ in range(100):
            # Generate random start and end frames
            start = random.randint(0, 2**h - 1)
            end = start + random.randint(0, 2**h - 1 - start)
            
            # Get the keys for this range
            keys = deriv.get_channel_keys(start, end)
            
            # Test random frames within the range
            for _ in range(5):
                frame = random.randint(start, end)
                
                # Get key using both methods
                direct_key = deriv.get_frame_key(frame).key
                cover_key = deriv.get_frame_key_from_cover(keys, frame)
                
                # Keys should match
                self.assertEqual(direct_key, cover_key)
            
            # Test that frames outside range raise an exception
            # Test that frames outside range raise an exception
            with self.assertRaises(Exception):
                deriv.get_frame_key_from_cover(keys, start - 1)
            
            with self.assertRaises(Exception):
                deriv.get_frame_key_from_cover(keys, end + 1)
                
            # Test 100 random frames outside the range
            for _ in range(100):
                # Randomly choose between before start or after end
                if random.randint(0,1) == 0:
                    # Test frame before start
                    if start > 0:
                        frame = random.randint(0, start-1)
                        with self.assertRaises(Exception):
                            deriv.get_frame_key_from_cover(keys, frame)
                else:
                    # Test frame after end 
                    if end < 2**h - 1:
                        frame = random.randint(end+1, 2**h - 1)
                        with self.assertRaises(Exception):
                            deriv.get_frame_key_from_cover(keys, frame)


if __name__ == '__main__':
    unittest.main()