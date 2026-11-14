pub mod event;
pub mod resource;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
use std::str::FromStr;

pub const POOL_ADDRESS_STR: &str =
    "0x796900ebe1a1a54ff9e932f19c548f5c1af5c6e7d34965857ac2f7b1d1ab2cbf";

const POOL_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(POOL_ADDRESS_STR).unwrap());

const CONTRACT_ADDRESS: Lazy<HexAddress> = Lazy::new(|| {
    HexAddress::from_str("0x16fe2df00ea7dde4a63409201f7f4e536bde7bb7335526a35d05111e68aa322c")
        .unwrap()
});
