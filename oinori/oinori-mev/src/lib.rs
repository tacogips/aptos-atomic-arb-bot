pub mod account;
pub mod account_address;
pub mod aggregator;
pub mod coins;
pub mod difference;
pub mod engine;
pub mod envs;
pub mod hasher;
pub mod ledger;
pub mod metrics;
mod path;
pub mod read_db;
pub mod resource;
pub mod swap;
pub mod transaction;
pub mod transaction_stream;
pub mod validator;

use crate::ledger::get_ledger_info_via_rpc;
use crate::swap::SwapRouterPool;
use anyhow::{anyhow, Result};
use aptos_config::config::NodeConfig;
use aptos_config::config::StorageDirPaths;
use aptos_data_client::client::AptosDataClient;
use aptos_logger::prelude::*;
use aptos_mempool::MempoolClientSender;

use aptos_api_types::AsConverter;
use aptos_db::AptosDB;
use aptos_storage_interface::{state_view::LatestDbStateCheckpointView, DbReader, DbReaderWriter};
use aptos_types::chain_id::ChainId;
use aptos_vm::data_cache::AsMoveResolver;
use futures::channel::mpsc;
use futures::SinkExt;
use parking_lot::RwLock;
use std::env;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::runtime::{Builder, Runtime};
use transaction::*;

pub fn start_oinori_mev(
    _chain_id: ChainId,
    aptos_net_data_client: AptosDataClient,
    db_reader: Arc<dyn DbReader>,
    mempool_client_sender: MempoolClientSender,
) -> Result<Runtime> {
    info!("========== OINORI MEV ENGINE STARTING ==========");
    info!("[OINORI] Initializing MEV runtime...");

    let oinori_mev_runtime = Builder::new_multi_thread()
        .thread_name_fn(|| {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
            format!("oinori-mev-{}", id)
        })
        .disable_lifo_slot()
        .enable_all()
        .build()
        .map_err(|err| anyhow!("Failed to create mev steaming client {}", err))?;

    let rt_handle = oinori_mev_runtime.handle();

    let envs = envs::OinoriEnvs::load().unwrap();

    rt_handle.spawn(mev_transaction_handler(
        aptos_net_data_client,
        db_reader,
        mempool_client_sender,
        envs,
    ));

    info!("---- mev sever start -----");
    Ok(oinori_mev_runtime)
}

async fn mev_transaction_handler(
    aptos_net_data_client: AptosDataClient,
    db_reader: Arc<dyn DbReader>,
    mempool_client_sender: MempoolClientSender,
    envs: envs::OinoriEnvs,
) {
    info!("[OINORI] Starting MEV transaction handler...");
    info!("[OINORI] Starting stream service notifier");

    let state_view = db_reader.clone().latest_state_checkpoint_view().unwrap();

    let (mut tx_sink, transaction_rx) = mpsc::unbounded::<Vec<Option<UserTransactionLog>>>();

    info!("[OINORI] Node ID: {}", &envs.node_id);
    info!("[OINORI] RPC URL: {}", &envs.rpc_url);
    info!("[OINORI] Dry run mode: {}", envs.oinori_mev_dry_run);
    info!("[OINORI] MEV engine starting for node: {}", &envs.node_id);

    let swap_router_pool = Arc::new(RwLock::new(SwapRouterPool::default()));
    let swap_router_pool_for_setup = swap_router_pool.clone();

    let mev_engine = Arc::new(RwLock::new(engine::Engine::new(
        transaction::builder::mainnet_chain_id(),
        mempool_client_sender,
        Arc::clone(&db_reader),
        &envs,
    )));
    let mev_engine_for_setup = mev_engine.clone();

    let db_reader_for_setup = Arc::clone(&db_reader);

    let envs_for_setup = envs.clone();
    let _setup_and_keep_sync_engine_jh = tokio::task::spawn(async {
        engine::setup_and_keep_sync_engine(
            envs_for_setup,
            mev_engine_for_setup,
            swap_router_pool_for_setup,
            db_reader_for_setup,
        )
        .await;
    });

    let aptos_net_data_client_for_mev_engine = aptos_net_data_client.clone();
    let _start_mev_engine_jh = tokio::task::spawn(async {
        engine::start_mev_engine(
            transaction_rx,
            mev_engine,
            swap_router_pool,
            aptos_net_data_client_for_mev_engine,
        )
        .await
    });

    let (oinori_tx_output_stream_sender, mut oinori_tx_output_stream_receiver) = mpsc::unbounded();

    let max_flying_request_num_for_stream = 5;

    let additional_tx_fetch_size = 2;
    let max_additional_tx_fetch_size = 7;

    let stream_tick_duration_nono_sec = 1_000_000; // nano sec

    let latest_remote_status = get_ledger_info_via_rpc(&envs.rpc_url).await.unwrap();

    let tx_stream_status = Arc::new(RwLock::new(
        transaction_stream::TransactionOutputStreamStatus::new(
            max_flying_request_num_for_stream,
            latest_remote_status.version,
        ),
    ));

    let node_id_for_stream = envs.node_id.clone();
    let _start_mev_tx_stream_jh = tokio::task::spawn(async move {
        transaction_stream::start_tx_stream(
            node_id_for_stream,
            aptos_net_data_client,
            additional_tx_fetch_size,
            (true, max_additional_tx_fetch_size),
            stream_tick_duration_nono_sec,
            tx_stream_status,
            oinori_tx_output_stream_sender,
        )
        .await
    });

    info!("--- starting stream transaction stream --- ");

    loop {
        match oinori_tx_output_stream_receiver.try_next() {
            Ok(Some(transaction_output_list_with_proof)) => {
                let transactions = {
                    let move_converter = state_view.as_converter(db_reader.clone(), None);

                    let tx_iter = UserTransactionLog::iter_from_tx_proof(
                        None,
                        transaction_output_list_with_proof,
                        &move_converter,
                    );

                    let transactions: Result<Vec<Option<UserTransactionLog>>> =
                        tx_iter.into_iter().collect();
                    transactions
                };
                match transactions {
                    Err(e) => error!("failed to collect transactions {e:?}"),
                    Ok(transactions) => {
                        if let Err(e) = tx_sink.send(transactions).await {
                            error!("tx send error {e:?} ");
                        }
                    },
                }
            },
            Err(_e) => { /* pass */ },
            Ok(None) => {
                info!("oinori meve txn channel closed.");
                break;
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::coins::CoinAmount;
    use std::path::PathBuf;

    pub fn create_aptos_db_test(node_config: &NodeConfig) -> (Arc<AptosDB>, DbReaderWriter) {
        let (aptos_db, db_rw) = DbReaderWriter::wrap(
            AptosDB::open(
                StorageDirPaths::from_path(&node_config.storage.dir()),
                true, /* readonly */
                node_config.storage.storage_pruner_config,
                node_config.storage.rocksdb_configs,
                node_config.storage.enable_indexer,
                node_config.storage.buffered_state_target_items,
                node_config.storage.max_num_nodes_per_lru_cache_shard,
                None,
            )
            .map_err(|err| anyhow!("DB failed to open {}", err))
            .unwrap(),
        );
        (aptos_db, db_rw)
    }

    const DFEAULT_CONFIG_PATH: &str = "./oinori-configs/mainnet-config/fullnode.yaml";

    pub fn load_node_config() -> NodeConfig {
        let config_path: PathBuf = env::var("NODE_CONFIG_PATH")
            .unwrap_or_else(|_| DFEAULT_CONFIG_PATH.to_string())
            .into();

        NodeConfig::load_from_path(&config_path).unwrap_or_else(|error| {
            panic!(
                "Failed to parse node config file! Given file path: {:?}. Error: {:?}",
                config_path.display(),
                error
            )
        })
    }

}
