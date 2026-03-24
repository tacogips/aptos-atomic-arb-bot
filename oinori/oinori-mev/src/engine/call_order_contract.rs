pub use super::find_route::*;

use aptos_logger::prelude::*;
use super::Engine;
use crate::account::get_account_via_rpc;
use crate::transaction::builder::*;
use anyhow::{anyhow, Result};
use aptos_data_client::client::AptosDataClient;
use aptos_mempool::MempoolClientRequest;
use aptos_storage_interface::DbReader;
use aptos_types::account_address::AccountAddress;
use aptos_types::mempool_status::MempoolStatusCode;

use chrono::DateTime;
use chrono::FixedOffset;
use chrono::Utc;
use futures::channel::oneshot;
use parking_lot::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

static CONTRACT_CALL_LOCKED: AtomicBool = AtomicBool::new(false);
static BROADCAST_PEER_NUM_AT_ONCE: usize = 2;

pub fn get_new_contract_locked() -> bool {
    CONTRACT_CALL_LOCKED.load(std::sync::atomic::Ordering::Acquire)
}

fn set_contract_call_locked(value: bool) {
    CONTRACT_CALL_LOCKED.store(value, std::sync::atomic::Ordering::Release);
}

pub async fn call_contacat(
    mev_engine: Arc<RwLock<Engine>>,
    version: u64,
    contract_args: ContractArgs,
    gas_unit_price: u64,
    use_loan: bool,
    max_gas_amount: Option<u64>,
    aptos_net_data_client: &AptosDataClient,
) -> Result<(Option<String>, Option<String>)> {
    if get_new_contract_locked() {
        return Ok((None, Some("contact call is locked ".to_string())));
    }

    let contract_called_at = Utc::now();
    let tx_broadcast_peers =
        aptos_net_data_client.choose_peer_for_oinori_tx_send(BROADCAST_PEER_NUM_AT_ONCE)?;

    if tx_broadcast_peers.is_empty() {
        return Err(anyhow!(
            "no peer to broadcast. tryed to choise {} peers.",
            BROADCAST_PEER_NUM_AT_ONCE
        ));
    }

    let (req_sender, callback) = oneshot::channel();

    let (node_id, signed_transaction, mut mempool_client_sender, tx_timeout_sec) = {
        //println!(
        //    "----- try write start 1 call contract {}",
        //    datetime_to_jst(Utc::now())
        //); //TODO(debug)
        let mut mev_engine_reader = mev_engine.read();
        //println!("----- try write finish 1 call contract");

        let seq_no = mev_engine_reader.sender_seq_no();
        let tx_timeout_sec = mev_engine_reader.tx_timeout_sec;

        let node_id = mev_engine_reader.node_id.clone();

        let bid_info = TxBidInfo::new(
            GasUnitPrice(gas_unit_price),
            seq_no,
            tx_timeout_sec,
            max_gas_amount,
        );

        let signed_transaction = if use_loan {
            mev_engine_reader
                .base_transaction_payload_with_loan
                .sign_with_args(
                    bid_info,
                    Some(contract_args.contract_func_args),
                    Some(contract_args.contract_func_types),
                )
                .unwrap()
        } else {
            mev_engine_reader
                .base_transaction_payload_no_loan
                .sign_with_args(
                    bid_info,
                    Some(contract_args.contract_func_args),
                    Some(contract_args.contract_func_types),
                )
                .unwrap()
        };

        (
            node_id,
            signed_transaction,
            mev_engine_reader.mempool_client_sender.clone(),
            tx_timeout_sec,
        )
    };

    use futures::SinkExt;
    mempool_client_sender
        .send(MempoolClientRequest::SubmitTransaction(
            signed_transaction.clone(),
            req_sender,
            Some(tx_broadcast_peers.clone()),
        ))
        .await?;

    let tx_hash_hex = signed_transaction.committed_hash().to_hex_literal();
    match callback.await? {
        Ok((mempool_status, request_result)) => {
            if mempool_status.code == MempoolStatusCode::Accepted {
                // start tracing the transaction

                let mev_for_update = mev_engine.clone();
                let db_reader = mev_engine.read().db_reader.clone();
                let current_sender_accont = mev_engine
                    .read()
                    .transaction_sender_with_seq_no
                    .as_ref()
                    .unwrap()
                    .sender_account_address()
                    .clone();

                let current_sender_seq_no = mev_engine
                    .read()
                    .transaction_sender_with_seq_no
                    .as_ref()
                    .unwrap()
                    .seq_no()
                    .clone();

                let tx_hash_hex_for_trace_tx = tx_hash_hex.clone();
                let node_id_for_trace_tx = node_id.clone();

                set_contract_call_locked(true); // lock subsequent contract calls
                let _ = tokio::task::spawn(async move {
                    start_check_transaction(
                        node_id_for_trace_tx,
                        mev_for_update,
                        db_reader,
                        current_sender_accont,
                        current_sender_seq_no,
                        tx_hash_hex_for_trace_tx,
                        tx_timeout_sec,
                    )
                    .await
                });

                info!(
                    "tracing start. target_version:{}, tx hash: {} broadcast peers:{:?}, account:{}, seq_no:{}",
                    version,
                    tx_hash_hex.clone(),
                    tx_broadcast_peers,
                    current_sender_accont.to_hex_literal(),
                    current_sender_seq_no.inner()
                );
            } else {
                error!(
                    "invalid mempool respone target_version:{},status:{} result:{:?} , submit takes {:?} us , broadcast peers:{:?}",
                    version,
                    mempool_status,
                    request_result,
                    (Utc::now() - contract_called_at).num_microseconds(),
                    tx_broadcast_peers
                );
            }

            Ok((Some(tx_hash_hex), None))
        },
        Err(e) => {
            error!(
                "send transaction error {e:?}, submit takes {:?} us",
                (Utc::now() - contract_called_at).num_microseconds()
            );

            Err(e)
        },
    }
}

pub async fn start_check_transaction(
    node_id: String,

    mev_engine: Arc<RwLock<Engine>>,
    db_reader: Arc<dyn DbReader>,
    sender_account_address: AccountAddress,
    current_sender_seq_no: AccountSeqNo,
    tx_hash_hex: String,
    tx_timeout_sec: u64,
) {
    let mut update_timer = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let rpc_url = mev_engine.read().rpc_url.clone();

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(tx_timeout_sec)); //11 sec

    tokio::pin!(timeout);

    loop {
        tokio::select! {
             _ = &mut timeout =>{
                info!(
                    "tracing tx timed out. tx hash: {}, current_seq_no: {}, account: {}",
                    tx_hash_hex, current_sender_seq_no.inner(), sender_account_address.to_hex_literal()
                );

                match get_account_via_rpc(sender_account_address.clone(), &rpc_url).await {
                    Ok(latest_account) => {
                        info!(
                            "tracing tx timed out and set seq no as latest account. tx hash: {}, new seq no: {}",
                            tx_hash_hex, latest_account.sequence_number
                        );
                        mev_engine.write().set_seq_no(latest_account.sequence_number);
                    },
                    Err(e) => {
                        warn!(
                            "tracing tx timed out and failed to get latest account. tx hash: {}, current_seq_no: {}, err: {e:?}",
                            tx_hash_hex, current_sender_seq_no.inner()
                        );
                    }
                }

                info!("unlock contract call by timeout");
                set_contract_call_locked(false);
                println!("======= finished check transaction with timedout and error ");
                return

             },

             _check_tx_at = update_timer.tick() =>{
                 match crate::read_db::get_tx_by_hash(&tx_hash_hex, db_reader.clone(), None) {
                    Ok(tx) => match tx {
                        Some(tx) => {
                            info!(
                                "tx found. tx hash: {}, version: {}, current_seq_no: {}",
                                tx_hash_hex, tx.version, current_sender_seq_no.inner()
                            );

                            mev_engine.write().increase_seq_no();

                            info!("unlock contract call by tx found");
                            set_contract_call_locked(false);
                            return
                        },
                        None => {
                            debug!(
                                "tx not found. tx hash: {}, current_seq_no: {}",
                                tx_hash_hex, current_sender_seq_no.inner()
                            );
                        }
                    },
                    Err(e) => {
                        warn!(
                            "get_tx_by_hash() error {}. tx hash: {}, current_seq_no: {}",
                            e, tx_hash_hex, current_sender_seq_no.inner()
                        );

                        match get_account_via_rpc(sender_account_address.clone(), &rpc_url).await {
                            Ok(latest_account) => {
                                info!(
                                    "tracing tx error and set seq no as latest account. tx hash: {}, new seq no: {}",
                                    tx_hash_hex, latest_account.sequence_number
                                );
                                mev_engine.write().set_seq_no(latest_account.sequence_number);
                            },
                            Err(e) => {
                                warn!(
                                    "tracing tx error and failed to get latest account. tx hash: {}, current_seq_no: {}, err: {e:?}",
                                    tx_hash_hex, current_sender_seq_no.inner()
                                );
                            }
                        }

                        info!("unlock contract call by error");
                        set_contract_call_locked(false);
                        return
                    }
                 }
             }
        }
    }
}

pub fn datetime_to_jst(dt: DateTime<Utc>) -> DateTime<FixedOffset> {
    dt.with_timezone(&chrono::offset::FixedOffset::east(9 * 3600))
}
