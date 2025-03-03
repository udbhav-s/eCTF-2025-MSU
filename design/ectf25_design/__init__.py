import json
from Crypto.Random import get_random_bytes
from Crypto.PublicKey import ECC
from typing import TypedDict, Dict, Tuple, List
from dataclasses import dataclass

class Secrets(TypedDict):
    channels: Dict[int, str]  # Maps channel IDs to hex-encoded 16-byte secrets
    decoder_dk: str  # Hex-encoded 32-byte decoder key
    host_key: str  # PEM-encoded Ed25519 host key

@dataclass
class ChannelTreeNode:
    node_num: int # The number of a node in a level-order traversal of the tree
    key: bytes

@dataclass
class ChannelKeyDerivation:
    root: bytes
    height: int = 4

    def get_channel_node_cover(self, node_num: int) -> Tuple[int, int]:
        """Given a node representing a derivation key for a channel's
        frames with 64-bit timestamps (2^64 possible frames),
        determine the range of frames it can decode

        :param node_num: Number of the node according to level-order traversal of tree
        """
        d = node_num.bit_length() - 1       # int( log2 node_num ) for node depth
        level_index = node_num - 2**d       # Index of the node at its level
        span = 2**(self.height - d)         # Number of leaves covered a node at depth d
        skipped = span * level_index        # Number of leaves covered by siblings before node
        node_cover = [skipped, skipped + span - 1]
        
        return tuple(node_cover)

    def get_channel_nodes_cover(self, nodes: List[ChannelTreeNode]) -> Tuple[int, int]:
        """Given a list of nodes, gives minimum and maximum timestamp they can decode
        """
        if not len(nodes):
            raise Exception("Cannot determine cover for empty list of nodes!")
        
        cover = list(self.get_channel_node_cover(nodes[0]))
        for node in nodes[1:]:
            node_cover = self.get_channel_node_cover(node)
            cover[0] = min(cover[0], node_cover[0])
            cover[1] = max(cover[1], node_cover[1])

        return tuple(cover)
    
    def get_covering_nodes(self, start: int, end: int) -> List[ChannelTreeNode]:
        """Returns a list of node numbers that cover the given range of frames (start and end inclusive)
        """
        nodes = []
        decode_range = (start, end)

        assert(start >= 0 and start <= 2**64 - 1 and end >= 0 and end <= 2**64 - 1 and start <= end)

        get_parent = lambda x: x // 2
        get_left_child = lambda x: x * 2
        # get_depth = lambda x: x.bit_length() - 1
        in_range = lambda r1, r2: r1[0] >= r2[0] and r1[1] <= r2[1] 

        # Leaf n is node 2**height + n in a level-order traversal
        start_node = start + 2**self.height

        descending = False
        iter_node = start_node
        while not (len(nodes) > 0 and decode_range == self.get_channel_nodes_cover(nodes)):
            if not descending:
                parent = iter_node
                while in_range(self.get_channel_node_cover(get_parent(parent)), decode_range):
                    parent = get_parent(parent)
                
                nodes.append(parent)

                # We hit the root node, cover everything
                if parent == 1: break

                # Move to right sibling once we can't go higher
                # Invariant: if iter_node is rightmost for level, decode_range should be covered already
                iter_node = parent + 1

                # If the right sibling's cover is out of range, we need to start descending
                if not in_range(self.get_channel_node_cover(iter_node), decode_range):
                    descending = True
            else:
                assert(get_left_child(iter_node) < 2**(self.height + 1), "Invariant: Left child should not go out of tree")

                iter_node = get_left_child(iter_node)
                while not in_range(self.get_channel_node_cover(iter_node), decode_range):
                    iter_node = get_left_child(iter_node)
                
                nodes.append(iter_node)
                
                # Move to right sibling
                iter_node += 1

        return nodes


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

    host_key = ECC.generate(curve='Ed25519')
    host_key_pem = host_key.export_key(format='PEM')

    # Create the secrets object
    secrets: Secrets = {
        "channels": channel_secrets,
        "decoder_dk": decoder_dk,
        "host_key": host_key_pem
    }

    return json.dumps(secrets).encode()

# if __name__ == '__main__':
#     test = ChannelTreeNode(node_num=1, key=b"0000")
#     print(get_channel_node_cover(test))   