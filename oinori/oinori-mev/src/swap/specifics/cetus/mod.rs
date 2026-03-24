pub mod event;
pub mod resource;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
use std::str::FromStr;

pub const POOL_ADDRESS_STR: &str =
    "0xec42a352cc65eca17a9fa85d0fc602295897ed6b8b8af6a6c79ef490eb8f9eba";

pub const POOL_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(POOL_ADDRESS_STR).unwrap());
