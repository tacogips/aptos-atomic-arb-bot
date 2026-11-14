mod private_key;

use anyhow::Result;
use aptos_crypto::HashValue;
use aptos_types::account_address::AccountAddress;

pub use private_key::*;

fn adjust_hex_address(hex_address: &str) -> String {
    let hex_address = if hex_address.starts_with("0x") {
        &hex_address[2..]
    } else {
        hex_address
    };
    format!("{:0>64}", hex_address)
}

//TODO use HexAdderss
pub fn hex_to_account_address(hex_address: &str) -> Result<AccountAddress> {
    let target_address = AccountAddress::from_hex(&adjust_hex_address(hex_address))?;
    Ok(target_address)
}

pub fn hex_to_hash_value(hex_address: &str) -> Result<HashValue> {
    let hash = HashValue::from_hex(&adjust_hex_address(hex_address))?;
    Ok(hash)
}

#[derive(Debug)]
pub struct HexAdderss(String);
impl HexAdderss {
    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

#[cfg(test)]
mod test {

    use super::*;
    #[test]
    fn test_func_name() {
        let account_address = adjust_hex_address("0x1");
        assert_eq!(
            account_address,
            "0000000000000000000000000000000000000000000000000000000000000001".to_string()
        )
    }
}
