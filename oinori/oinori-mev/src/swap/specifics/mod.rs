pub mod anime;
pub mod apto_swap;
pub mod aux;
pub mod cetus;
pub mod liquid;
pub mod obric;
pub mod pancake;
pub mod thala;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub enum SwapContract {
    Liquid,
    Aux,
    Anime,
    Pancake,
    Cetus,
    Obric,
    AptoSwap,
    Thala,
}

impl SwapContract {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Liquid,
            Self::Aux,
            Self::Anime,
            Self::Pancake,
            Self::Cetus,
            Self::Obric,
            Self::AptoSwap,
            Self::Thala,
        ]
    }

    pub fn pool_address(&self) -> &str {
        match self {
            Self::Liquid => liquid::POOL_ADDRESS_STR,
            Self::Aux => aux::POOL_ADDRESS_STR,
            Self::Anime => anime::POOL_ADDRESS_STR,
            Self::Pancake => pancake::POOL_ADDRESS_STR,
            Self::Cetus => cetus::POOL_ADDRESS_STR,
            Self::Obric => obric::POOL_ADDRESS_STR,
            Self::AptoSwap => apto_swap::POOL_ADDRESS_STR,
            Self::Thala => thala::POOL_ADDRESS_STR,
        }
    }
}
