mod stream_status;

use aptos_logger::prelude::*;
use crate::engine::*;
use aptos_data_client::client::AptosDataClient;
use aptos_types::transaction::{TransactionOutputListWithProof, Version};
use chrono::offset::FixedOffset;
use chrono::{DateTime, Utc};
use futures::channel::mpsc::UnboundedSender;
use futures::SinkExt;
use parking_lot::RwLock;
use rand::Rng;
use std::sync::Arc;
pub use stream_status::*;
use tokio::sync::mpsc::{
    self as tokio_mpsc, UnboundedReceiver as TokioUnboundedReceiver,
    UnboundedSender as TokioUnboundedSender,
};
use tokio::time::{interval, Duration};

const CLIENT_TIMEOUT_MS: u64 = 45;
pub async fn start_tx_stream<'a>(
    node_id: String,
    aptos_net_data_client: AptosDataClient,
    additional_tx_fetch_size: usize,
    stochastic_additiolan_tx_fetch_size: (bool, usize),

    tick_nano_sec: u64,
    tx_stream_status: Arc<RwLock<TransactionOutputStreamStatus>>,
    transaction_sender: UnboundedSender<TransactionOutputListWithProof>,
) {
    tokio::task::spawn(async move {
        let stream_tick_duration = Duration::from_nanos(tick_nano_sec);
        let mut stream_tick_interval = interval(stream_tick_duration);

        let (next_fetch_notifier_tx, mut next_fetch_notifier_rx): (
            TokioUnboundedSender<()>,
            TokioUnboundedReceiver<()>,
        ) = tokio_mpsc::unbounded_channel();

        loop {
            tokio::select! {
                 _ = stream_tick_interval.tick() => {
                    run_stream_fetch(
                        node_id.clone(),
                        aptos_net_data_client.clone(),
                        additional_tx_fetch_size,
                        stochastic_additiolan_tx_fetch_size,
                        tx_stream_status.clone(),
                        transaction_sender.clone(),
                        next_fetch_notifier_tx.clone(),
                    );

                 },


                 next_fetch_notify = next_fetch_notifier_rx.recv() => {
                    if let Some(_) = next_fetch_notify {
                        run_stream_fetch(
                            node_id.clone(),
                            aptos_net_data_client.clone(),
                            additional_tx_fetch_size,
                            stochastic_additiolan_tx_fetch_size,
                            tx_stream_status.clone(),
                            transaction_sender.clone(),
                            next_fetch_notifier_tx.clone(),
                        );
                    }
                 },


            }
        }
    });
}

fn run_stream_fetch(
    node_id: String,

    aptos_net_data_client: AptosDataClient,
    additional_tx_fetch_size: usize,
    stochastic_additiolan_tx_fetch_size: (bool, usize),

    tx_stream_status: Arc<RwLock<TransactionOutputStreamStatus>>,
    transaction_sender: UnboundedSender<TransactionOutputListWithProof>,
    next_fetch_notifier: TokioUnboundedSender<()>,
) {
    if !crate::engine::get_boostrap_synced() {
        return;
    }

    let mut transaction_sender_cloned = transaction_sender.clone();

    match new_stream_request_sender(aptos_net_data_client.clone(), tx_stream_status.clone()) {
        None => { /*  frying request reached max  */ },
        Some(new_stream_request_sender) => {
            let _ = tokio::task::spawn(async move {
                let fetch_start_at = chrono::Utc::now();
                let additional_tx_fetch_size = match stochastic_additiolan_tx_fetch_size {
                    (true, stocatstick_max_range) => {
                        let mut rng = rand::thread_rng();
                        rng.gen_range(
                            additional_tx_fetch_size,
                            additional_tx_fetch_size + stocatstick_max_range,
                        )
                    },
                    (false, _) => additional_tx_fetch_size,
                };

                match new_stream_request_sender
                    .send_transaction_request(FetchTransactionRequest::new(
                        additional_tx_fetch_size,
                        CLIENT_TIMEOUT_MS,
                    ))
                    .await
                {
                    Ok((transaciton_list, fetched_version, latest_tx_version_before_update)) => {
                        if let Err(e) = transaction_sender_cloned.send(transaciton_list).await {
                            error!("transaction stream send error {:?}", e);
                        }
                    },
                    Err(e) => {
                        let err_msg = e.to_string();
                        if !err_msg.contains("cannot be satisfied")
                            && !err_msg.contains("Rpc timed out")
                            && !err_msg.contains("dropped response")
                        {
                            println!(
                                "---- oinori stream request error {}{}",
                                datetime_to_jst(Utc::now()),
                                e
                            );
                        }
                    },
                };

                next_fetch_notifier.send(())
            });
        },
    }
}

pub fn datetime_to_jst(dt: DateTime<Utc>) -> DateTime<FixedOffset> {
    dt.with_timezone(&chrono::offset::FixedOffset::east(9 * 3600))
}
