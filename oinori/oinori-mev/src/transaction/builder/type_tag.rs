use crate::account_address::hex_to_account_address;
use crate::coins::CoinId;
use crate::swap::amm::AmmCurveType;
use crate::swap::specifics::SwapContract;
use anyhow::Result;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{StructTag, TypeTag};

#[derive(Debug, Eq, PartialEq)]
pub enum FunctionTypeTag {
    Coin(CoinId),
    CurveType(Option<CurveTypeTag>),
    String,
    U8,
}

impl FunctionTypeTag {
    pub fn to_type_tag(&self) -> Result<TypeTag> {
        let tag = match &self {
            Self::U8 => TypeTag::U8,
            Self::String => TypeTag::Struct(Box::new(string_struct_tag())),
            Self::Coin(coin) => TypeTag::Struct(Box::new(coin_struct_tag(coin)?)),
            Self::CurveType(curve_type) => match curve_type {
                Some(curve_type) => TypeTag::Struct(Box::new(curve_type.to_struct_tag())),
                None => TypeTag::U8,
            },
        };
        Ok(tag)
    }

    pub fn coin(coin_id: CoinId) -> Self {
        Self::Coin(coin_id)
    }

    pub fn curve_type(cuve_type_tag: Option<CurveTypeTag>) -> Self {
        Self::CurveType(cuve_type_tag)
    }

    pub fn string() -> Self {
        Self::String
    }

    pub fn u8() -> Self {
        Self::U8
    }
}

fn coin_struct_tag(coin: &CoinId) -> Result<StructTag> {
    let (address, module, name) = coin.as_address_and_identifiers()?;
    Ok(StructTag {
        address,
        module,
        name,
        type_args: vec![],
    })
}

fn string_struct_tag() -> StructTag {
    StructTag {
        address: hex_to_account_address("0x1").unwrap(),
        module: Identifier::new("string").unwrap(),
        name: Identifier::new("String").unwrap(),
        type_args: vec![],
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CurveTypeTag {
    LiquidUncorrelated,
    LiquidStable,
}

impl CurveTypeTag {
    fn to_struct_tag(&self) -> StructTag {
        match &self {
            Self::LiquidUncorrelated => StructTag {
                address: hex_to_account_address(
                    "0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12",
                )
                .unwrap(),
                module: Identifier::new("curves").unwrap(),
                name: Identifier::new("Uncorrelated").unwrap(),
                type_args: vec![],
            },
            Self::LiquidStable => StructTag {
                address: hex_to_account_address(
                    "0x190d44266241744264b964a37b8f09863167a12d3e70cda39376cfb4e3561e12",
                )
                .unwrap(),
                module: Identifier::new("curves").unwrap(),
                name: Identifier::new("Stable").unwrap(),
                type_args: vec![],
            },
        }
    }
}

pub fn convert_swap_curve_type_tag(
    swap_contract: &SwapContract,
    curve_type: &AmmCurveType,
) -> Option<CurveTypeTag> {
    if swap_contract == &SwapContract::Liquid {
        match curve_type {
            AmmCurveType::LiquidStable => Some(CurveTypeTag::LiquidStable),
            AmmCurveType::UniswapV2 => Some(CurveTypeTag::LiquidUncorrelated),
            _ => None,
        }
    } else {
        None
    }
}
