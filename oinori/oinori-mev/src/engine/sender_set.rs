use crate::account_address::hex_to_account_address;
use crate::transaction::builder::TransactionSender;
use aptos_types::account_address::AccountAddress;

use anyhow::{anyhow, Error as AnyhowError, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct SenderSet {
    pub sender_address: SenderAddressAndPrivateKey,
}

impl SenderSet {
    pub fn load(sender_set_id: &str) -> Result<Self> {
        let sender_set_file = crate::path::PROJECT_DIR
            .get_file(format!("resources/sender_set/{}.toml", sender_set_id))
            .ok_or(anyhow!("failed to sender_set {}", sender_set_id))?;
        let result: Self = toml::from_slice(sender_set_file.contents())?;
        Ok(result)
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename = "sender_address")]
pub struct SenderAddressAndPrivateKey {
    pub hex_address: String,
    pub hex_private_key: String,
}

impl SenderAddressAndPrivateKey {
    pub fn as_account_address(&self) -> Result<AccountAddress> {
        hex_to_account_address(&self.hex_address)
    }
}

impl From<&TransactionSender> for SenderAddressAndPrivateKey {
    fn from(sender: &TransactionSender) -> Self {
        let hex_private_key = format!("0x{}", hex::encode(sender.private_key().to_bytes()));
        let hex_address = format!("0x{}", sender.address().to_hex());

        Self {
            hex_private_key,
            hex_address,
        }
    }
}

impl TryFrom<&SenderAddressAndPrivateKey> for TransactionSender {
    type Error = AnyhowError;
    fn try_from(sender: &SenderAddressAndPrivateKey) -> Result<Self, Self::Error> {
        let transaction_sender = TransactionSender::from_address_and_key_strs(
            &sender.hex_address,
            &sender.hex_private_key,
        )?;
        Ok(transaction_sender)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_load_address() {
        //
        let addresses = SenderSet::load("default").unwrap();
        // anonymized original account address in sanitized fixture
        assert_eq!(
            addresses.sender_address.hex_address,
            "anonymous".to_string()
        );
    }
}
