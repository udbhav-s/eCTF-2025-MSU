import json
from Crypto.Random import get_random_bytes
from Crypto.PublicKey import ECC
from typing import TypedDict, Dict

class Secrets(TypedDict):
    channels: Dict[int, str]  # Maps channel IDs to hex-encoded 16-byte secrets
    decoder_dk: str  # Hex-encoded 32-byte decoder key
    host_key: str  # PEM-encoded Ed25519 host key

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

    # NOTE: if you choose to use JSON for your file type, you will not be able to
    # store binary data, and must either use a different file type or encode the
    # binary data to hex, base64, or another type of ASCII-only encoding
    return json.dumps(secrets).encode()