pub mod builder;
pub mod sender;
use anyhow::Result;
use aptos_logger::prelude::*;
use aptos_types::{
    block_info::BlockInfo,
    transaction::{
        ExecutionStatus, Transaction, TransactionInfo, TransactionOutput,
        TransactionOutputListWithProof,
    },
    vm_status::AbortLocation,
    write_set::WriteSet,
};
use chrono::{DateTime, NaiveDateTime, Utc};

use serde::{Deserialize, Serialize};

use aptos_api_types::{
    Address as HexAddress, Event, MoveConverter,
    TransactionPayload as SerializableTransactionPayload, WriteSetChange,
};
use aptos_storage_interface::state_view::DbStateView;

pub struct SimpleBlockInfo {
    pub timestamp_usecs: u64,
    pub epoch: u64,
    pub round: u64,
    pub block_hash: String,
}
impl From<&BlockInfo> for SimpleBlockInfo {
    fn from(block_info: &BlockInfo) -> Self {
        SimpleBlockInfo {
            timestamp_usecs: block_info.timestamp_usecs(),
            epoch: block_info.epoch(),
            round: block_info.round(),
            block_hash: block_info.id().to_hex_literal(),
        }
    }
}

pub struct TxVersionCounter {
    version: u64,
}

impl TxVersionCounter {
    pub fn new(version: u64) -> Self {
        Self { version }
    }
}
impl Iterator for TxVersionCounter {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let result = self.version;
        self.version += 1;
        Some(result)
    }
}

fn abort_location_to_str(loc: &AbortLocation) -> String {
    match loc {
        AbortLocation::Module(mid) => {
            format!("{}::{}", mid.address().to_hex_literal(), mid.name())
        },
        _ => loc.to_string(),
    }
}

fn explain_vm_status(
    move_converter: &MoveConverter<DbStateView>,
    status: &ExecutionStatus,
) -> String {
    match status {
            ExecutionStatus::MoveAbort { location, code, info } => match &location {
                AbortLocation::Module(_) => {
                    info.as_ref().map(|i| {
                        format!("Move abort in {}: {}({:#x}): {}", abort_location_to_str(location), i.reason_name, code, i.description)
                    }).unwrap_or_else(|| {
                        format!("Move abort in {}: {:#x}", abort_location_to_str(location), code)
                    })
                }
                AbortLocation::Script => format!("Move abort: code {:#x}", code),
            },
            ExecutionStatus::Success => "Executed successfully".to_owned(),
            ExecutionStatus::OutOfGas => "Out of gas".to_owned(),
            ExecutionStatus::ExecutionFailure {
                location,
                function,
                code_offset,
            } => {
                let func_name = match location {
                    AbortLocation::Module(module_id) => move_converter.explain_function_index(module_id, function)
                        .map(|name| format!("{}::{}", abort_location_to_str(location), name))
                        .unwrap_or_else(|_| format!("{}::<#{} function>", abort_location_to_str(location), function)),
                    AbortLocation::Script => "script".to_owned(),
                };
                format!(
                    "Execution failed in {} at code offset {}",
                    func_name, code_offset
                )
            }
            ExecutionStatus::MiscellaneousError(code) => {
                code.map_or(
                    "Move bytecode deserialization / verification failed, including entry function not found or invalid arguments".to_owned(),
                    |e| format!(
                        "Transaction Executed and Committed with Error {:#?}", e
                    ),
                )
            }
        }
}

pub struct UserTransactionLogIter<'a> {
    simple_block_info: Option<SimpleBlockInfo>,
    move_converter: &'a MoveConverter<'a, DbStateView>,
    tx_iter: TxIterType,
}

use std::iter::Zip;
use std::vec::IntoIter;
type TxIterType = Zip<
    Zip<IntoIter<(Transaction, TransactionOutput)>, IntoIter<TransactionInfo>>,
    TxVersionCounter,
>;

impl<'a> Iterator for UserTransactionLogIter<'a> {
    type Item = Result<Option<UserTransactionLog>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.tx_iter.next() {
            None => None,
            Some((((tx, output), tx_info), version)) => {
                let user_tx = UserTransactionLog::new(
                    self.simple_block_info.as_ref(),
                    version,
                    tx,
                    output,
                    tx_info,
                    self.move_converter,
                );
                Some(user_tx)
            },
        }
    }
}

pub struct BlockTimestampMicroSec(u64);

impl BlockTimestampMicroSec {
    pub fn as_datetime(&self) -> DateTime<Utc> {
        let naive_datetime = NaiveDateTime::from_timestamp_opt(
            (self.0 / 1_000_000) as i64,
            (self.0 % 1_000_000 * 1_000) as u32,
        )
        .unwrap_or_default();

        DateTime::from_utc(naive_datetime, Utc)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserTransactionLog {
    pub block_timestamp_usecs: Option<u64>,
    pub block_epoch: Option<u64>,
    pub block_round: Option<u64>,

    pub tx_hash: String,
    pub version: u64,
    pub gas_used: u64,
    pub success: bool,
    pub vm_status: String,
    pub events: Vec<Event>,
    pub changes: Vec<WriteSetChange>,
    pub sender_address: String,
    pub sequence_number: u64,
    pub payload: SerializableTransactionPayload,
    pub max_gas_amount: u64,
    pub gas_unit_price: u64,
    pub expiration_timestamp_secs: u64,
}

impl UserTransactionLog {
    pub fn block_timestamp(&self) -> Option<BlockTimestampMicroSec> {
        self.block_timestamp_usecs
            .map(|timestamp| BlockTimestampMicroSec(timestamp))
    }

    pub fn iter_from_tx_proof<'a>(
        simple_block_info: Option<SimpleBlockInfo>,
        tx_with_proof: TransactionOutputListWithProof,
        move_converter: &'a MoveConverter<'a, DbStateView>,
    ) -> UserTransactionLogIter<'a> {
        let TransactionOutputListWithProof {
            transactions_and_outputs, //: Vec<(Transaction, TransactionOutput)>,
            proof,                    //: TransactionInfoListWithProof,
            first_transaction_output_version,
        } = tx_with_proof;

        if transactions_and_outputs.len() != proof.transaction_infos.len() {
            panic!("-----!!! invalid tx output and proof")
        }

        let versions = TxVersionCounter::new(first_transaction_output_version.unwrap_or(0));

        let tx_iter = transactions_and_outputs
            .into_iter()
            .zip(proof.transaction_infos)
            .zip(versions);

        UserTransactionLogIter {
            simple_block_info,
            move_converter,
            tx_iter,
        }
    }

    pub fn new(
        block_info: Option<&SimpleBlockInfo>,
        version: u64,
        transaction: Transaction,
        transactoin_output: TransactionOutput,
        tx_info: TransactionInfo,
        move_converter: &MoveConverter<'_, DbStateView>,
    ) -> Result<Option<UserTransactionLog>> {
        if let Transaction::UserTransaction(user_tx) = transaction {
            let TransactionInfo::V0(v0_info) = tx_info;
            let (tx_hash, tx_status) = (
                v0_info.transaction_hash().to_hex_literal(),
                v0_info.status(),
            );

            //TODO _auxiliary has error detail. we can manage them if we need more deep information.
            let (write_set, events, gas_used, _status, _auxiliary) = transactoin_output.unpack();

            let events = match move_converter.try_into_events(&events) {
                Ok(events) => events,
                Err(e) => {
                    error!("error convert tx event {e:?}");
                    vec![]
                },
            };

            let write_set_changes = write_set_changes_from_write_set(write_set, move_converter);
            let expiration_timestamp_secs = user_tx.expiration_timestamp_secs();
            let sequence_number = user_tx.sequence_number();

            let max_gas_amount = user_tx.max_gas_amount();
            let gas_unit_price = user_tx.gas_unit_price();

            let sender_address: HexAddress = user_tx.sender().into();

            let raw_txn = user_tx.into_raw_transaction();

            match move_converter.try_into_transaction_payload(raw_txn.into_payload()) {
                Ok(payload) => Ok(Some(UserTransactionLog {
                    block_timestamp_usecs: block_info.map(|block_info| block_info.timestamp_usecs),
                    block_epoch: block_info.map(|block_info| block_info.epoch),
                    block_round: block_info.map(|block_info| block_info.round),

                    tx_hash,
                    version,

                    gas_used,
                    success: tx_status == &ExecutionStatus::Success,
                    vm_status: explain_vm_status(move_converter, tx_status),

                    events,
                    changes: write_set_changes,
                    sender_address: sender_address.to_string(),
                    sequence_number,
                    payload,
                    max_gas_amount,
                    gas_unit_price,
                    expiration_timestamp_secs,
                })),
                Err(e) => {
                    error!("invalid tx payload tx hash {}, {e:?}", tx_hash);
                    Err(e)
                },
            }
        } else {
            Ok(None)
        }
    }
}

pub fn write_set_changes_from_write_set(
    write_set: WriteSet,
    move_converter: &MoveConverter<DbStateView>,
) -> Vec<WriteSetChange> {
    write_set
        .into_iter()
        .filter_map(|(state_key, write_ope)| {
            move_converter
                .try_into_write_set_changes(state_key, write_ope)
                .ok()
        })
        .flatten()
        .collect()
}
