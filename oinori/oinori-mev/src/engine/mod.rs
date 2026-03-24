pub mod engine_analysis;
mod cache;
mod call_order_contract;
mod find_route;
mod gas_calculation;
mod mev_runner;
mod route_filter;
mod sender_set;
mod setup_and_keep_sync;
mod sync_engine;

use aptos_logger::prelude::*;
use crate::account_address::hex_to_account_address;
use crate::aggregator;
use crate::envs::*;
use crate::hasher::*;
use crate::swap::{SwapContract, SwapResource, SwapRouterPool};
use crate::transaction::builder::*;
use crate::transaction::*;
use anyhow::Result;
use aptos_data_client::client::AptosDataClient;
use aptos_storage_interface::DbReader;

use aptos_api_types::WriteSetChange;
use aptos_mempool::MempoolClientSender;

use aptos_types::account_address::AccountAddress;
use aptos_types::chain_id::ChainId;
pub use setup_and_keep_sync::*;

use cache::*;

use chrono::Utc;
pub use find_route::*;

use futures::channel::mpsc;
use parking_lot::RwLock;
use sender_set::*;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use url::Url;

const TX_TIMEOUT_SEC: u64 = 1;

const DEFAULT_MAX_GAS_LIMIT: u64 = 250; // !!! important if max gaslimit * gas price over your account
                                        // balance, the tx will be ignored

static LAST_USER_TX_GTT_AT: AtomicU64 = AtomicU64::new(0);

static ALREADY_MEVED_VERSION: AtomicU64 = AtomicU64::new(0);

fn get_already_meved_version() -> u64 {
    ALREADY_MEVED_VERSION.load(std::sync::atomic::Ordering::Acquire)
}

pub fn set_already_meved_version(version: u64) {
    ALREADY_MEVED_VERSION.store(version, std::sync::atomic::Ordering::Release);
}

pub fn get_last_user_tx_got_at_nano_sec() -> u64 {
    LAST_USER_TX_GTT_AT.load(std::sync::atomic::Ordering::Acquire)
}

fn set_last_user_tx_got_at_nano_sec(at_nano_sec: u64) {
    LAST_USER_TX_GTT_AT.store(at_nano_sec, std::sync::atomic::Ordering::Release);
}

#[derive(Debug)]
pub struct SenderWithSeqNo {
    pub sender: TransactionSender,
    pub seq_no: AccountSeqNo,
}

impl SenderWithSeqNo {
    pub fn new(sender: TransactionSender, seq_no: u64) -> Self {
        Self {
            sender,
            seq_no: AccountSeqNo::new(seq_no),
        }
    }
    pub fn sender_account_address(&self) -> &AccountAddress {
        self.sender.address()
    }

    pub fn seq_no(&self) -> &AccountSeqNo {
        &self.seq_no
    }

    pub fn increase_seq_no(&mut self) {
        self.seq_no.increase()
    }

    pub fn set_seq_no(&mut self, seq_no: AccountSeqNo) {
        self.seq_no = seq_no
    }
}

pub struct Engine {
    rpc_url: Url,
    node_id: String,

    mempool_client_sender: MempoolClientSender,

    transaction_sender_address_and_key: SenderAddressAndPrivateKey,
    transaction_sender_with_seq_no: Option<SenderWithSeqNo>,
    sender_apt_balance: Option<u64>,
    available_swaps: HashSet<SwapContract>,
    base_transaction_payload_with_loan: TransactionBase,
    base_transaction_payload_no_loan: TransactionBase,

    tx_timeout_sec: u64,
    recent_fired_mev_cache: RecentFiredMevCache,
    dry_run: bool,
    disabled: bool,
    db_reader: Arc<dyn DbReader>,
}

impl Engine {
    pub fn new(
        chain_id: ChainId,
        mempool_client_sender: MempoolClientSender,
        db_reader: Arc<dyn DbReader>,
        envs: &OinoriEnvs,
    ) -> Self {
        let sender_set = SenderSet::load(&envs.oinori_sender_set_id).unwrap();
        let transaction_sender_address_and_key = sender_set.sender_address;

        info!(
            " using {} for sender in engine",
            transaction_sender_address_and_key.hex_address
        );

        let transaction_sender =
            TransactionSender::try_from(&transaction_sender_address_and_key).unwrap();
        println!("-- buidling hippo aggegator script start ...");
        let base_transaction_payload_with_loan = aggregator::build_hippo_aggregator_script_v2(
            chain_id.clone(),
            transaction_sender,
            Some(DEFAULT_MAX_GAS_LIMIT),
            &envs.contract_dir,
        )
        .unwrap();

        println!("-- buidling hippo aggegator script end ...");

        let transaction_sender =
            TransactionSender::try_from(&transaction_sender_address_and_key).unwrap();
        let base_transaction_payload_no_loan =
            aggregator::build_hippo_aggregator_script_v2_no_loan(
                chain_id.clone(),
                transaction_sender,
                Some(DEFAULT_MAX_GAS_LIMIT),
                &envs.contract_dir,
            )
            .unwrap();
        let recent_fired_mev_cache = RecentFiredMevCache::new(None);

        let disabled = envs.oinori_mev_disable;
        let dry_run = envs.oinori_mev_dry_run;
        Self {
            rpc_url: envs.rpc_url.clone(),
            node_id: envs.node_id.clone(),
            mempool_client_sender,
            transaction_sender_with_seq_no: None,
            sender_apt_balance: None,
            available_swaps: HashSet::default(),
            transaction_sender_address_and_key,
            base_transaction_payload_with_loan,
            base_transaction_payload_no_loan,
            tx_timeout_sec: TX_TIMEOUT_SEC,
            recent_fired_mev_cache,
            dry_run,
            disabled,
            db_reader,
        }
    }

    pub fn transaction_sender_with_seq_no(&self) -> Option<&SenderWithSeqNo> {
        self.transaction_sender_with_seq_no.as_ref()
    }

    fn init_sender_with_seq_no(&mut self, seq_no: u64) {
        let transaction_sender =
            TransactionSender::try_from(&self.transaction_sender_address_and_key).unwrap();
        let new_transaction_sender_with_seq_no = SenderWithSeqNo::new(transaction_sender, seq_no);
        self.transaction_sender_with_seq_no
            .replace(new_transaction_sender_with_seq_no);
    }

    fn sender_address(&self) -> AccountAddress {
        self.transaction_sender_with_seq_no
            .as_ref()
            .unwrap()
            .sender
            .address()
            .clone()
    }

    fn sender_seq_no(&self) -> AccountSeqNo {
        self.transaction_sender_with_seq_no.as_ref().unwrap().seq_no
    }

    fn update_available_swaps(&mut self, available_swaps: HashSet<SwapContract>) {
        self.available_swaps = available_swaps;
    }

    fn increase_seq_no(&mut self) {
        if let Some(sender) = self.transaction_sender_with_seq_no.as_mut() {
            sender.increase_seq_no()
        }
    }

    fn set_seq_no(&mut self, seq_no: u64) {
        if let Some(sender) = self.transaction_sender_with_seq_no.as_mut() {
            sender.set_seq_no(AccountSeqNo(seq_no))
        }
    }
}

pub async fn run_mev(
    node_id: &str,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: Arc<RwLock<SwapRouterPool>>,
    version: u64,
    target_tx_block_timestamp: Option<BlockTimestampMicroSec>,
    tx_sender_address: AccountAddress,
    changes: Vec<WriteSetChange>,
    aptos_net_data_client: &AptosDataClient,
) -> Result<()> {
    let mev_receipt_tx_at = Utc::now();

    let (
        swap_resources,
        sender_account_address,
        seq_no_at_contract,
        account_apt_balance,
        dry_run,
        disabled,
    ) = {
        let mev_engine_reader = mev_engine.read();
        (
            SwapResource::from_changes(&changes, &mev_engine_reader.available_swaps)?,
            mev_engine_reader.sender_address(),
            mev_engine_reader.sender_seq_no(),
            mev_engine_reader.sender_apt_balance,
            mev_engine_reader.dry_run,
            mev_engine_reader.disabled,
        )
    };

    if disabled {
        return Ok(());
    }

    let result = if swap_resources.is_empty() {
        Ok(())
    } else {
        mev_runner::run_mev(
            &node_id,
            mev_runner::AccountInfoForMev {
                sender_account_address,
                seq_no_at_contract,
                account_apt_balance,
            },
            version,
            tx_sender_address,
            swap_resources,
            mev_engine.clone(),
            swap_router_pool.clone(),
            &aptos_net_data_client,
            mev_receipt_tx_at,
            dry_run,
        )
        .await
    };

    result
}

pub async fn start_mev_engine(
    mut rx: mpsc::UnboundedReceiver<Vec<Option<UserTransactionLog>>>,
    mev_engine: Arc<RwLock<Engine>>,
    swap_router_pool: Arc<RwLock<SwapRouterPool>>,
    aptos_net_data_client: AptosDataClient,
) {
    let node_id = mev_engine.read().node_id.clone();
    info!("--- start mev engine ---");
    loop {
        match rx.try_next() {
            Ok(Some(user_tx_logs)) => {
                for each_tx in user_tx_logs.into_iter() {
                    match each_tx {
                        Some(each_tx) => {
                            let synced_version = setup_and_keep_sync::get_engine_synced_version();

                            if synced_version >= each_tx.version || synced_version == 0 {
                                continue;
                            }

                            let already_meved = get_already_meved_version();

                            if already_meved >= each_tx.version {
                                println!("---- skip. already meved version {already_meved} the tx version :{}", each_tx.version);
                                continue;
                            }

                            set_already_meved_version(each_tx.version);
                            set_last_user_tx_got_at_nano_sec(Utc::now().timestamp_nanos() as u64);

                            if let Err(e) = run_mev(
                                &node_id,
                                mev_engine.clone(),
                                swap_router_pool.clone(),
                                each_tx.version,
                                each_tx.block_timestamp(),
                                hex_to_account_address(&each_tx.sender_address).unwrap(),
                                each_tx.changes,
                                &aptos_net_data_client,
                            )
                            .await
                            {
                                error!("run mev error {:?} version{}", e, each_tx.version);
                            }

                            setup_and_keep_sync::set_engine_synced_version(each_tx.version)
                        },
                        None => {},
                    }
                }
            },
            Err(_e) => {},
            Ok(None) => {
                info!("oinori txn channel closed.");
                break;
            },
        }
    }
}
