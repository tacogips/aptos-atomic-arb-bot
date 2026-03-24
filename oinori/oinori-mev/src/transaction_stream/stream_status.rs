use crate::transaction::TxVersionCounter;
use anyhow::Result;
use aptos_data_client::client::AptosDataClient;
use aptos_data_client::interface::AptosDataClientInterface;
use aptos_types::transaction::{TransactionOutputListWithProof, Version};
use chrono::offset::FixedOffset;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::sync::Arc;

pub struct TransactionOutputStreamStatus {
    pub latest_fetched_version: u64,
    flying_request_num: usize,
    max_flying_request_num: usize,
}
impl TransactionOutputStreamStatus {
    pub fn new(max_flying_request_num: usize, initial_tx_version: u64) -> Self {
        Self {
            latest_fetched_version: initial_tx_version,
            flying_request_num: 0,
            max_flying_request_num,
        }
    }
}

pub fn new_stream_request_sender(
    aptos_net_data_client: AptosDataClient,
    tx_stream_status: Arc<RwLock<TransactionOutputStreamStatus>>,
) -> Option<TransactionOutputStreamRequestSender> {
    //println!("---------- read3  start");
    let reader = tx_stream_status.read();
    if reader.flying_request_num > reader.max_flying_request_num {
        //println!("---------- read3  end -1");
        None
    } else {
        //println!("---------- read3  end -2");
        Some(TransactionOutputStreamRequestSender {
            requested_num: 0,
            tx_stream_status: tx_stream_status.clone(),
            aptos_net_data_client,
            fetched_tx_versions_by_this_sender: vec![],
        })
    }
}

pub struct TransactionOutputStreamRequestSender {
    aptos_net_data_client: AptosDataClient,
    requested_num: usize,
    fetched_tx_versions_by_this_sender: Vec<u64>,
    tx_stream_status: Arc<RwLock<TransactionOutputStreamStatus>>,
}

pub struct FetchTransactionRequest {
    pub additional_get_tx_num: usize,
    timout_ms: u64,
}

impl FetchTransactionRequest {
    pub fn new(additional_get_tx_num: usize, timout_ms: u64) -> Self {
        Self {
            additional_get_tx_num,
            timout_ms,
        }
    }

    fn as_param_versions(&self, latest_fetched_version: u64) -> (u64, u64, u64) {
        let start_version = latest_fetched_version + 1;
        let end_version = start_version + self.additional_get_tx_num as u64;
        let epoch_version = end_version;
        (start_version, end_version, epoch_version)
    }
}

impl TransactionOutputStreamRequestSender {
    pub async fn send_transaction_request(
        mut self,
        request: FetchTransactionRequest,
    ) -> Result<(TransactionOutputListWithProof, Vec<u64>, u64)> {
        self.requested_num += 1;
        //println!("---------- write1 start");
        self.tx_stream_status.write().flying_request_num += 1;

        //println!("---------- write1 end");

        //println!("---------- read1  start");
        let (start, end, epoch) =
            request.as_param_versions(self.tx_stream_status.read().latest_fetched_version);

        //println!("---------- read1  end");

        let client_response = self
            .aptos_net_data_client
            .get_transaction_outputs_with_proof(epoch, start, end, request.timout_ms);

        let response = client_response.await?.into_payload();
        self.fetched_tx_versions_by_this_sender = Self::tx_versions(&response);

        //println!("---------- read2  start");
        let latest_tx_version_before_update = self.tx_stream_status.read().latest_fetched_version;

        //println!("---------- read2  end");

        Ok((
            response,
            self.fetched_tx_versions_by_this_sender.clone(),
            latest_tx_version_before_update,
        ))
    }

    fn tx_versions(proof: &TransactionOutputListWithProof) -> Vec<u64> {
        match proof.first_transaction_output_version {
            None => {
                vec![]
            },
            Some(first_transaction_output_version) => {
                let version_convert = TxVersionCounter::new(first_transaction_output_version);
                version_convert
                    .take(proof.proof.transaction_infos.len())
                    .collect()
            },
        }
    }
}

impl Drop for TransactionOutputStreamRequestSender {
    fn drop(&mut self) {
        //println!("---------- write start on drop {:?}", chrono::Utc::now());
        let mut writer = self.tx_stream_status.write();

        //println!("---------- write start writing  {:?}", chrono::Utc::now());
        if let Some(last_fetch_tx_version) = self.fetched_tx_versions_by_this_sender.last() {
            //println!("---------- write update vresion {:?}", chrono::Utc::now());
            let got_at = chrono::Utc::now();
            if *last_fetch_tx_version > writer.latest_fetched_version {
                writer.latest_fetched_version = *last_fetch_tx_version;
            } else {
                //
            }
        }

        writer.flying_request_num -= self.requested_num;

        //println!("---------- write exist on drop {:?}", chrono::Utc::now());
    }
}

pub fn datetime_to_jst(dt: DateTime<Utc>) -> DateTime<FixedOffset> {
    dt.with_timezone(&chrono::offset::FixedOffset::east(9 * 3600))
}
