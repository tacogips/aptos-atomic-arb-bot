mod pool_type;
pub mod resource;
mod weight;

pub use pool_type::*;
pub use resource::*;
pub use weight::*;

use aptos_api_types::Address as HexAddress;
use once_cell::sync::Lazy;
pub use resource::*;
use std::str::FromStr;

pub const POOL_ADDRESS_STR: &str =
    "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af";

const POOL_ADDRESS: Lazy<HexAddress> =
    Lazy::new(|| HexAddress::from_str(POOL_ADDRESS_STR).unwrap());
