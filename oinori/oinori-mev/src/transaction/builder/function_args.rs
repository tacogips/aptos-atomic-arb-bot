use crate::account_address::{hex_to_account_address, HexAdderss};
use anyhow::{anyhow, Error as AnyhowError, Result};
use aptos_types::transaction::TransactionArgument;

use move_core_types::language_storage::TypeTag;
use move_core_types::parser::{parse_transaction_argument, parse_type_tag};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct FunctionArg {
    #[serde(rename = "type")]
    arg_type: FunctionArgType,
    value: String,
}

impl FunctionArg {
    // removek
    pub fn as_type_and_value(&self) -> Result<(TypeTag, Vec<u8>)> {
        as_type_and_value(&self.arg_type, &self.value)
    }

    pub fn as_transaction_arg(&self) -> Result<TransactionArgument> {
        if self.arg_type == FunctionArgType::String {
            return Ok(TransactionArgument::U8Vector(
                bcs::to_bytes(&self.value)?.to_vec(),
            ));
        }

        let tx_arg = match self.arg_type {
            FunctionArgType::String => {
                TransactionArgument::U8Vector(bcs::to_bytes(&self.value)?.to_vec())
            },

            FunctionArgType::U8 => TransactionArgument::U8(self.value.parse()?),
            FunctionArgType::U64 => TransactionArgument::U64(self.value.parse()?),
            FunctionArgType::U128 => TransactionArgument::U128(self.value.parse()?),
            FunctionArgType::Bool => TransactionArgument::Bool(self.value.parse()?),
            FunctionArgType::Address => {
                TransactionArgument::Address(hex_to_account_address(&self.value)?)
            },

            FunctionArgType::Vector => {
                return Ok(TransactionArgument::U8Vector(hex::decode(&self.value)?));
            },
        };

        Ok(tx_arg)
    }

    pub fn string(value: String) -> Self {
        Self {
            arg_type: FunctionArgType::String,
            value,
        }
    }

    pub fn u8(value: u8) -> Self {
        Self {
            arg_type: FunctionArgType::U8,
            value: value.to_string(),
        }
    }

    pub fn u64(value: u64) -> Self {
        Self {
            arg_type: FunctionArgType::U64,
            value: value.to_string(),
        }
    }

    pub fn u128(value: u128) -> Self {
        Self {
            arg_type: FunctionArgType::U128,
            value: value.to_string(),
        }
    }

    pub fn bool(value: bool) -> Self {
        Self {
            arg_type: FunctionArgType::Bool,
            value: value.to_string(),
        }
    }

    pub fn address(value: HexAdderss) -> Self {
        Self {
            arg_type: FunctionArgType::Bool,
            value: value.to_string(),
        }
    }

    pub fn vector(value: Vec<u8>) -> Self {
        Self {
            arg_type: FunctionArgType::Vector,
            value: hex::encode(value),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum FunctionArgType {
    U8,
    U64,
    U128,
    Bool,
    String,
    Address,
    Vector,
}

impl TryFrom<&str> for FunctionArgType {
    type Error = AnyhowError;

    fn try_from(s: &str) -> std::result::Result<Self, Self::Error> {
        let v = match s {
            "u8" => Self::U8,
            "u64" => Self::U64,
            "u128" => Self::U128,
            "bool" => Self::Bool,
            "address" => Self::Address,
            "vector" => Self::Vector,
            "string" => Self::String,
            other => return Err(anyhow!("arg type {} is not supported", other)),
        };
        Ok(v)
    }
}

impl FunctionArgType {
    fn as_str(&self) -> &str {
        match self {
            Self::U8 => "u8",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Bool => "bool",
            Self::Address => "address",
            Self::Vector => "vector",
            Self::String => "string",
        }
    }
}

pub fn as_type_and_value(arg_type: &FunctionArgType, value: &str) -> Result<(TypeTag, Vec<u8>)> {
    if arg_type == &FunctionArgType::String {
        return Ok((
            TypeTag::Vector(Box::new(TypeTag::U8)),
            bcs::to_bytes(&value)?,
        ));
    }

    let arg_type = arg_type.as_str();
    let type_tag = parse_type_tag(arg_type)?;
    let tx_arg = parse_transaction_argument(value)?;

    let arg_bytes = match &type_tag {
        TypeTag::Bool => {
            if let TransactionArgument::Bool(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },
        TypeTag::U8 => {
            if let TransactionArgument::U8(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },

        TypeTag::U16 => {
            if let TransactionArgument::U16(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },

        TypeTag::U32 => {
            if let TransactionArgument::U32(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },

        TypeTag::U64 => {
            if let TransactionArgument::U64(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },
        TypeTag::U128 => {
            if let TransactionArgument::U128(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },

        TypeTag::U256 => {
            if let TransactionArgument::U256(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },
        TypeTag::Address => {
            if let TransactionArgument::Address(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },
        TypeTag::Vector(_tags) => {
            if let TransactionArgument::U8Vector(v) = tx_arg {
                bcs::to_bytes(&v)?
            } else {
                return Err(anyhow!("unmatch type:{:?}, {:?}", type_tag, tx_arg));
            }
        },
        TypeTag::Signer => return Err(anyhow!("signer type is not available for argument")),
        TypeTag::Struct(_tag) => return Err(anyhow!("struct type is not available for argument")),
    };

    Ok((type_tag, arg_bytes))
}
