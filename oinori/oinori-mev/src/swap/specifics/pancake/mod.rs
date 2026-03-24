pub mod resource;

pub mod event;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
use std::str::FromStr;

pub const POOL_ADDRESS_STR: &str =
    "0xc7efb4076dbe143cbcd98cfaaa929ecfc8f299203dfff63b95ccb6bfe19850fa";

pub const POOL_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(POOL_ADDRESS_STR).unwrap());
