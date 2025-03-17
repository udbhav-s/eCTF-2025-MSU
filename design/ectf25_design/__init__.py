import json
from Crypto.Random import get_random_bytes
from Crypto.PublicKey import ECC
from Crypto.Hash import MD5, SHA512
from Crypto.Protocol.KDF import HKDF
from typing import TypedDict, Dict, Tuple, List
from dataclasses import dataclass


class Secrets(TypedDict):
    channels: Dict[str, str]  # Maps channel IDs to hex-encoded 16-byte secrets
    decoder_dk: str  # Hex-encoded 32-byte decoder key
    host_key: str  # Ed25519 host key in DER encoded as hex


@dataclass
class ChannelTreeNode:
    node_num: int  # The number of a node in a level-order traversal of the tree
    key: bytes


@dataclass
class ChannelKeyDerivation:
    root: bytes
    height: int = 64

    def get_channel_node_cover(self, node_num: int) -> Tuple[int, int]:
        """Given a node representing a derivation key for a channel's
        frames with 64-bit timestamps (2^64 possible frames),
        determine the range of frames it can decode

        :param node_num: Number of the node according to level-order traversal of tree
        """
        d = node_num.bit_length() - 1  # int( log2 node_num ) for node depth
        level_index = node_num - 2**d  # Index of the node at its level
        span = 2 ** (self.height - d)  # Number of leaves covered a node at depth d
        skipped = span * level_index  # Number of leaves covered by siblings before node
        node_cover = [skipped, skipped + span - 1]

        return tuple(node_cover)

    def get_channel_nodes_cover(self, nodes: List[int]) -> Tuple[int, int]:
        """Given a list of nodes, gives minimum and maximum timestamp they can decode"""
        if not len(nodes):
            raise Exception("Cannot determine cover for empty list of nodes!")

        cover = list(self.get_channel_node_cover(nodes[0]))
        for node in nodes[1:]:
            node_cover = self.get_channel_node_cover(node)
            cover[0] = min(cover[0], node_cover[0])
            cover[1] = max(cover[1], node_cover[1])

        return tuple(cover)

    def get_covering_nodes(self, start: int, end: int) -> List[int]:
        """Returns a list of node numbers that cover the given range of frames (start and end inclusive)"""
        nodes = []
        decode_range = (start, end)

        assert (
            start >= 0
            and start <= 2**64 - 1
            and end >= 0
            and end <= 2**64 - 1
            and start <= end
        )

        get_parent = lambda x: x // 2
        get_left_child = lambda x: x * 2
        # get_depth = lambda x: x.bit_length() - 1
        in_range = lambda r1, r2: r1[0] >= r2[0] and r1[1] <= r2[1]

        # Leaf n is node 2**height + n in a level-order traversal
        start_node = start + 2**self.height

        descending = False
        iter_node = start_node
        while not (
            len(nodes) > 0 and decode_range == self.get_channel_nodes_cover(nodes)
        ):
            if not descending:
                parent = iter_node
                while in_range(
                    self.get_channel_node_cover(get_parent(parent)), decode_range
                ):
                    parent = get_parent(parent)

                nodes.append(parent)

                # We hit the root node, cover everything
                if parent == 1:
                    break

                # Move to right sibling once we can't go higher
                # Invariant: if iter_node is rightmost for level, decode_range should be covered already
                iter_node = parent + 1

                # If the right sibling's cover is out of range, we need to start descending
                if not in_range(self.get_channel_node_cover(iter_node), decode_range):
                    descending = True
            else:
                # Invariant: Left child should not go out of tree
                assert get_left_child(iter_node) < 2 ** (self.height + 1)

                iter_node = get_left_child(iter_node)
                while not in_range(
                    self.get_channel_node_cover(iter_node), decode_range
                ):
                    iter_node = get_left_child(iter_node)

                nodes.append(iter_node)

                # Move to right sibling
                iter_node += 1

        return nodes

    def get_left_subkey(self, key: bytes):
        return MD5.new(key + b"L").digest()

    def get_right_subkey(self, key: bytes):
        return MD5.new(key + b"R").digest()

    def get_key_for_node(self, node_num: int) -> ChannelTreeNode:
        """Generate the key for a given node in the tree from the root key"""
        traversal = []
        n = node_num
        # Traverse to root
        while n > 1:
            traversal.append(n % 2)
            n = n // 2

        print(f"GETTING KEY FOR NODE f{node_num}")
        print(f"ROOT KEY: {self.root.decode()}")

        # Traverse from root, generating subkeys along the way
        curr_key = self.root
        for t in traversal[::-1]:
            if t == 0:
                curr_key = self.get_left_subkey(curr_key)
            else:
                curr_key = self.get_right_subkey(curr_key)
            print(f"NEXT KEY: {curr_key.decode()}")

        return ChannelTreeNode(node_num=node_num, key=curr_key)

    def get_channel_keys(self, start: int, end: int) -> List[ChannelTreeNode]:
        """Takes a timestamp range, and generates a list of tree nodes with keys that cover that range"""

        node_numbers = self.get_covering_nodes(start, end)
        nodes = []

        for node_num in node_numbers:
            nodes.append(self.get_key_for_node(node_num))

        return nodes

    def extend_key(self, key: bytes) -> bytes:
        """Extends 16-byte key to 32 by returning (k | H(k))"""
        return key + MD5.new(key).digest()

    def get_frame_key(self, frame_num: int) -> bytes:
        """Returns a 16-byte key to be used for encrypting a given frame, based on the hash tree derivation"""
        node_num = frame_num + 2**self.height
        node = self.get_key_for_node(node_num)
        return node.key


    def get_frame_key_from_cover(self, nodes: List[ChannelTreeNode], frame_num: int):
        """Given a cover of the tree and a frame to decode, verify that the frame can be decoded
        (Similar to what the Decoder will do)
        """
        node_num = frame_num + 2**self.height

        traversal = []
        n = node_num
        # Traverse to root
        while n > 1:
            traversal.append(n % 2)
            n = n // 2

        # Reverse to get traversal from root to leaf
        traversal = traversal[::-1]

        curr_node = 1
        node_one = [k for k in nodes if k.node_num is curr_node]
        closest_node = node_one[0] if len(node_one) else None
        closest_node_idx = 0 if closest_node is not None else None

        for i, branch in enumerate(traversal):
            idx = i + 1

            if branch == 0:
                curr_node = curr_node * 2
            else:
                curr_node = curr_node * 2 + 1

            found_nodes = [k for k in nodes if k.node_num == curr_node]
            if any(found_nodes):
                closest_node = found_nodes[0]
                closest_node_idx = idx

        # If we are unable to derive a key using a node from the list
        if closest_node is None:
            raise Exception("Could not derive a key from the given nodes")

        # Derive the key from the closest node found in subscription package
        curr_node = closest_node.node_num
        curr_key = closest_node.key
        for branch in traversal[closest_node_idx:]:
            if branch == 0:
                curr_node = curr_node * 2
                curr_key = self.get_left_subkey(curr_key)
            else:
                curr_node = curr_node * 2 + 1
                curr_key = self.get_right_subkey(curr_key)

        return curr_key


def get_decoder_key(decoder_dk: bytes, decoder_id: int):
    decoder_id_bytes = decoder_id.to_bytes(length=4, byteorder="little")
    return HKDF(
        master=decoder_dk, key_len=32, hashmod=SHA512, context=decoder_id_bytes, salt=""
    )


def gen_secrets(channels: list[int]) -> bytes:
    """Generate the contents secrets file

    This will be passed to the Encoder, ectf25_design.gen_subscription, and the build
    process of the decoder

    :param channels: List of channel numbers that will be valid in this deployment.
        Channel 0 is the emergency broadcast, which will always be valid and will
        NOT be included in this list

    :returns: Contents of the secrets file
    """

    channels.append(0)

    channel_secrets = {}
    for channel in channels:
        channel_secrets[channel] = get_random_bytes(16).hex()

    decoder_dk = get_random_bytes(32).hex()

    
    # Generate Ed25519 private key
    host_key = ECC.generate(curve="Ed25519")
    host_key_der = host_key.export_key(format='DER').hex()

    # Extract public key
    host_public_key = host_key.public_key()
    host_public_key_der = host_public_key.export_key(format="DER").hex()

    # Create the secrets object
    secrets: Secrets = {
        "channels": channel_secrets,
        "decoder_dk": decoder_dk,
        "host_key_priv": host_key_der,
        "host_key_pub": host_public_key_der,
    }

    return json.dumps(secrets).encode()


# if __name__ == "__main__":
#     test = ChannelTreeNode(node_num=1, key=b"0000")
#     print(get_channel_node_cover(test))
