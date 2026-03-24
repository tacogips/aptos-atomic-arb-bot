//pub mod change;
pub mod event;
pub mod flashloan;
pub mod resource;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
pub use resource::*;
use std::str::FromStr;

const CONTRACT_ADDRESS_STR: &str =
    "0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12";

pub const POOL_ADDRESS_STR: &str =
    "0x5a97986a9d031c4567e15b797be516910cfcb4156312482efc6a19c0a30c948";

const POOL_ADDRESS: Lazy<HexAddress> = Lazy::new(|| {
    HexAddress::from_str("0x05a97986a9d031c4567e15b797be516910cfcb4156312482efc6a19c0a30c948")
        .unwrap()
});

const CONTRACT_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(CONTRACT_ADDRESS_STR).unwrap());
