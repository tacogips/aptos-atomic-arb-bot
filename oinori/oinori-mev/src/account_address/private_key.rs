use anyhow::Result;
use aptos_crypto::ed25519::Ed25519PrivateKey;

fn hex_to_private_key_bytes(hex_private_key: &str) -> Result<Vec<u8>> {
    let hex_private_key = if hex_private_key.starts_with("0x") {
        &hex_private_key[2..]
    } else {
        hex_private_key
    };
    Ok(hex::decode(hex_private_key)?)
}

pub fn hex_to_private_key(hex_private_key: &str) -> Result<Ed25519PrivateKey> {
    let private_key_bytes = hex_to_private_key_bytes(hex_private_key)?;

    Ok(Ed25519PrivateKey::try_from(private_key_bytes.as_slice())?)
}
