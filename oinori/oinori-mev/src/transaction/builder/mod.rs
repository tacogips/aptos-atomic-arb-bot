mod entry_function;
mod function_args;
mod package_builder;
mod script;
mod type_tag;

use anyhow::Result;
use aptos_crypto::ed25519::Ed25519PrivateKey;
use aptos_sdk::types::LocalAccount;
use aptos_sdk::{transaction_builder::TransactionFactory, types::AccountKey};
use aptos_types::account_address::AccountAddress;
use aptos_types::chain_id::{ChainId, NamedChain};

use aptos_types::transaction::{SignedTransaction, TransactionPayload};
use std::ops::Deref;

pub use crate::account_address::{hex_to_account_address, hex_to_private_key};
pub use entry_function::*;
pub use function_args::*;
pub use package_builder::*;
pub use script::*;
pub use type_tag::*;

#[derive(Debug)]
pub struct TransactionSender {
    address: AccountAddress,
    account_key: AccountKey,
}

impl TransactionSender {
    pub fn new(address: AccountAddress, private_key: Ed25519PrivateKey) -> Self {
        Self {
            address,
            account_key: private_key.into(),
        }
    }

    pub fn from_address_and_key_strs(
        hex_account_address: &str,
        hex_private_key: &str,
    ) -> Result<Self> {
        let account_address = hex_to_account_address(hex_account_address)?;
        let private_key = hex_to_private_key(&hex_private_key)?;
        Ok(Self::new(account_address, private_key))
    }

    pub fn create_new_random() -> Self {
        let local_address = LocalAccount::generate(&mut rand_core::OsRng);
        Self::new(local_address.address(), local_address.private_key().clone())
    }

    pub fn address(&self) -> &AccountAddress {
        &self.address
    }

    pub fn private_key(&self) -> &Ed25519PrivateKey {
        self.account_key.private_key()
    }
}

fn after_n_sec_from_now(sec: u64) -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + sec
}

const DEFAULT_EXIRATION_SEC: u64 = 15;

pub fn mainnet_chain_id() -> ChainId {
    ChainId::new(NamedChain::MAINNET.id())
}

pub fn testnet_chain_id() -> ChainId {
    ChainId::new(NamedChain::TESTNET.id())
}

// tx witch doesn't have sequence no and gas price
#[derive(Debug)]
pub struct TransactionBase {
    sender: TransactionSender,
    factory: TransactionFactory,
    base_payload: BasePayload,
}

#[derive(Debug)]
pub enum BasePayload {
    EntryFunction(EntryFunctionCall),
    Script(ScriptCall),
    ModulePublish(TransactionPayload),
}

impl BasePayload {
    pub fn as_transaction_payload(
        &self,

        args: Option<Vec<FunctionArg>>,
        types: Option<Vec<FunctionTypeTag>>,
    ) -> Result<TransactionPayload> {
        match self {
            Self::EntryFunction(call) => call.as_transaction_payload(args, types),
            Self::Script(call) => call.as_transaction_payload(args, types),
            Self::ModulePublish(payload) => Ok(payload.clone()),
        }
    }
}

impl TransactionBase {
    pub fn new(
        sender: TransactionSender,
        base_payload: BasePayload,
        chain_id: ChainId,
        with_default_max_gas_amount: Option<u64>,
    ) -> TransactionBase {
        let factory = TransactionFactory::new(chain_id)
            .with_max_gas_amount(with_default_max_gas_amount.unwrap_or(5000));

        TransactionBase {
            sender,
            factory,
            base_payload,
        }
    }

    pub fn new_script(
        sender: TransactionSender,
        build_package_info: BuildPackageInfo,
        chain_id: Option<ChainId>,
        with_default_max_gas_amount: Option<u64>,
    ) -> Result<Self> {
        let chain_id = chain_id.unwrap_or(mainnet_chain_id());
        let script_call = build_package_info.build_script_call()?;

        Ok(Self::new(
            sender,
            BasePayload::Script(script_call),
            chain_id,
            with_default_max_gas_amount,
        ))
    }

    pub fn sign_with_args(
        &self,
        bid_info: TxBidInfo,
        args: Option<Vec<FunctionArg>>,
        types: Option<Vec<FunctionTypeTag>>,
    ) -> Result<SignedTransaction> {
        sign_transaction(&self, bid_info, args, types)
    }
}

#[derive(Debug)]
pub struct GasUnitPrice(pub u64);
impl GasUnitPrice {
    pub fn new_with_apt(_apt_value: u64) {}
}
impl Deref for GasUnitPrice {
    type Target = u64;
    fn deref(&self) -> &u64 {
        &self.0
    }
}
#[derive(Debug, Clone, Copy)]
pub struct AccountSeqNo(pub u64);
impl AccountSeqNo {
    pub fn new(no: u64) -> Self {
        Self(no)
    }
    pub fn increase(&mut self) {
        self.0 += 1;
    }
    pub fn inner(&self) -> u64 {
        self.0
    }
}
impl Deref for AccountSeqNo {
    type Target = u64;
    fn deref(&self) -> &u64 {
        &self.0
    }
}

#[derive(Debug)]
pub struct TxBidInfo {
    gas_unit_price: GasUnitPrice,
    account_sequence_number: AccountSeqNo,
    expire_in_sec: u64,
    max_gas_amonunt: Option<u64>,
}

impl TxBidInfo {
    pub fn new_with_default_expire(
        gas_price: GasUnitPrice,
        account_sequence_number: AccountSeqNo,
    ) -> Self {
        Self::new(
            gas_price,
            account_sequence_number,
            DEFAULT_EXIRATION_SEC,
            None,
        )
    }

    pub fn new(
        gas_unit_price: GasUnitPrice,
        account_sequence_number: AccountSeqNo,
        expire_in_sec: u64,
        max_gas_amonunt: Option<u64>,
    ) -> Self {
        Self {
            gas_unit_price,
            account_sequence_number,
            expire_in_sec,
            max_gas_amonunt,
        }
    }
}

fn sign_transaction(
    tx_base: &TransactionBase,
    bid_info: TxBidInfo,
    args: Option<Vec<FunctionArg>>,
    types: Option<Vec<FunctionTypeTag>>,
) -> Result<SignedTransaction> {
    let payload = tx_base.base_payload.as_transaction_payload(args, types)?;

    let mut tx_factory = tx_base.factory.clone();

    if let Some(max_gas_amonunt) = bid_info.max_gas_amonunt {
        tx_factory = tx_factory.with_max_gas_amount(max_gas_amonunt);
    }

    let raw_tx = tx_factory
        .payload(payload)
        .sender(tx_base.sender.address)
        .gas_unit_price(*bid_info.gas_unit_price)
        .sequence_number(*bid_info.account_sequence_number)
        .expiration_timestamp_secs(after_n_sec_from_now(bid_info.expire_in_sec))
        .build();
    let signe_tx = raw_tx
        .sign(
            tx_base.sender.account_key.private_key(),
            tx_base.sender.account_key.public_key().clone(),
        )?
        .into_inner();
    Ok(signe_tx)
}
