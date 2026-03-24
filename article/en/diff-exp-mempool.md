# Changes to the mempool package

The mempool is the blockchain transaction pool, responsible for broadcast and validation. In oinori, to speed up MEV transaction handling, the validation logic is simplified and an immediate broadcast feature is added.

---

## `mempool/Cargo.toml`

### Change: Add the chrono dependency
- **Change**: Added `chrono = { workspace = true }`
- **Purpose**: For timestamp handling, such as managing broadcast timing

```diff
@@ -48,6 +48,8 @@ thiserror = { workspace = true }
 tokio = { workspace = true }
 tokio-stream = { workspace = true }

+chrono = { workspace = true }
+
 [dev-dependencies]
```

---

## `mempool/src/core_mempool/transaction_store.rs`

### Change 1: Remove duplicate-transaction checks
- **Change**: Removed the duplicate transaction detection logic inside the `insert` method, about 50 lines
- **Purpose**: Speed up MEV transaction submission
- **Background**: In a normal mempool, if a transaction with the same account and sequence number already exists, updates are allowed only when the gas price is higher (gas bumping). In oinori, submitting our own MEV transactions as quickly as possible is the top priority, so this validation is skipped

```diff
@@ -226,55 +226,7 @@ impl TransactionStore {
         let txn_seq_num = txn.sequence_info.transaction_sequence_number;
         let acc_seq_num = txn.sequence_info.account_sequence_number;

-        // If the transaction is already in Mempool, we only allow the user to
-        // increase the gas unit price to speed up a transaction, but not the max gas.
-        if let Some(txns) = self.transactions.get_mut(&address) {
-            if let Some(current_version) = txns.get_mut(&txn_seq_num) {
-                if current_version.txn.payload() != txn.txn.payload() {
-                    return MempoolStatus::new(MempoolStatusCode::InvalidUpdate).with_message(
-                        "Transaction already in mempool with a different payload".to_string(),
-                    );
-                } else if current_version.txn.expiration_timestamp_secs()
-                    != txn.txn.expiration_timestamp_secs()
-                {
-                    return MempoolStatus::new(MempoolStatusCode::InvalidUpdate).with_message(
-                        "Transaction already in mempool with a different expiration timestamp"
-                            .to_string(),
-                    );
-                } else if current_version.txn.max_gas_amount() != txn.txn.max_gas_amount() {
-                    return MempoolStatus::new(MempoolStatusCode::InvalidUpdate).with_message(
-                        "Transaction already in mempool with a different max gas amount"
-                            .to_string(),
-                    );
-                } else if current_version.get_gas_price() < txn.get_gas_price() {
-                    if let Some(txn) = txns.remove(&txn_seq_num) {
-                        self.index_remove(&txn);
-                    };
-                    counters::CORE_MEMPOOL_GAS_UPGRADED_TXNS.inc();
-                } else if current_version.get_gas_price() > txn.get_gas_price() {
-                    return MempoolStatus::new(MempoolStatusCode::InvalidUpdate).with_message(
-                        "Transaction already in mempool with a higher gas price".to_string(),
-                    );
-                } else {
-                    counters::CORE_MEMPOOL_IDEMPOTENT_TXNS.inc();
-                    return MempoolStatus::new(MempoolStatusCode::Accepted);
-                }
-            }
-        }
-
-        if self.check_is_full_after_eviction(&txn, acc_seq_num) {
-            return MempoolStatus::new(MempoolStatusCode::MempoolIsFull).with_message(format!(
-                "Mempool is full. Mempool size: {}, Capacity: {}",
-                self.system_ttl_index.size(),
-                self.capacity,
-            ));
-        }
+        // oinori: skip duplicate tx check for performance

         self.clean_committed_transactions(&address, acc_seq_num);
```

### Change 2: Remove `check_is_full_after_eviction` and `check_txn_ready`
- **Change**: Removed both methods entirely, about 70 lines total
- **Purpose**: Skip eviction handling when the mempool is full and skip transaction readiness checks

```diff
@@ -348,82 +300,10 @@ impl TransactionStore {

-    fn check_is_full_after_eviction(
-        &mut self,
-        txn: &MempoolTransaction,
-        curr_sequence_number: u64,
-    ) -> bool {
-        if self.is_full() && self.check_txn_ready(txn, curr_sequence_number) {
-            // ... parking lot eviction logic (~35 lines) ...
-        }
-        self.is_full()
-    }
-
     fn is_full(&self) -> bool {
         self.system_ttl_index.size() >= self.capacity || self.size_bytes >= self.capacity_bytes
     }

-    fn check_txn_ready(&self, txn: &MempoolTransaction, curr_sequence_number: u64) -> bool {
-        // ... readiness check logic (~20 lines) ...
-    }
-
     fn log_ready_transaction(
```

---

## `mempool/src/shared_mempool/coordinator.rs`

### Change 1: Update the `MempoolClientRequest::SubmitTransaction` pattern match
- **Change**: Updated it to receive the third argument, `broadcast_peer_ids`

### Change 2: Immediate broadcast after transaction submission
- **Change**: Chained immediate broadcast processing with `.then()` after `process_client_transaction_submission`
- **Purpose**: Broadcast MEV transactions immediately to specific validator peers

```diff
@@ -173,7 +174,7 @@ async fn handle_client_request<NetworkClient, TransactionValidator>(
 {
     match request {
-        MempoolClientRequest::SubmitTransaction(txn, callback) => {
+        MempoolClientRequest::SubmitTransaction(txn, callback, broadcast_peer_ids) => {
             // This timer measures how long it took for the bounded executor to *schedule* the task.
             let _timer = counters::task_spawn_latency_timer(
@@ -187,13 +188,38 @@ async fn handle_client_request<NetworkClient, TransactionValidator>(
             smp.network_interface
                 .num_mempool_txns_received_since_peers_updated += 1;

+            let broadcast_peer_ids_cloned = broadcast_peer_ids.clone();
+            let mut cloned_smp = smp.clone();
             bounded_executor
-                .spawn(tasks::process_client_transaction_submission(
-                    smp.clone(),
-                    txn,
-                    callback,
-                    task_start_timer,
-                ))
+                .spawn(
+                    tasks::process_client_transaction_submission(
+                        smp.clone(),
+                        txn,
+                        callback,
+                        task_start_timer,
+                    )
+                    .then(move |prev_output| async move {
+                        // TODO (tacogips)oinori broadcast the tx immediately
+                        if let Some(broadcast_peer_ids) = broadcast_peer_ids_cloned {
+                            for (index, broadcast_peer_id) in
+                                broadcast_peer_ids.into_iter().enumerate()
+                            {
+                                let network_interface = &cloned_smp.network_interface.clone();
+                                let broadcast_result = network_interface
+                                    .execute_broadcast(
+                                        broadcast_peer_id.clone(),
+                                        false,
+                                        &mut cloned_smp,
+                                        false,
+                                    )
+                                    .await;
+                            }
+                        }
+
+                        prev_output
+                    }),
+                )
                 .await;
         },
```

---

## `mempool/src/shared_mempool/network.rs`

### Change 1: Skip ACK retry and backoff handling

```diff
@@ -328,16 +328,7 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf
         tasks::update_ack_counter(&peer, counters::RECEIVED_LABEL, retry, backoff);

-        if retry {
-            sync_state.broadcast_info.retry_messages.insert(message_id);
-        }
-
-        if backoff {
-            sync_state.broadcast_info.backoff_mode = true;
-        }
+        // oinori: skip retry and backoff handling
     }
```

### Change 2: Add the `do_check_peer_prioritized_oinori` parameter to `determine_broadcast_batch`

```diff
@@ -358,6 +349,7 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf
         peer: PeerNetworkId,
         scheduled_backoff: bool,
         smp: &mut SharedMempool<NetworkClient, TransactionValidator>,
+        do_check_peer_prioritized_oinori: bool,
     ) -> Result<
```

### Change 3: Force priority for oinori broadcasts

```diff
@@ -485,7 +476,8 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf
                         })
                         .collect()
                     } else {
-                        self.prioritized_peers_state
+                        let buckets_with_priorities: Vec<_> = self
+                            .prioritized_peers_state
                             .get_sender_buckets_for_peer(&peer)
                             .ok_or_else(|| {
                                 BroadcastError::PeerNotPrioritized(
@@ -495,7 +487,18 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf
                             })?
                             .clone()
                             .into_iter()
-                            .collect()
+                            .collect();
+
+                        if do_check_peer_prioritized_oinori {
+                            buckets_with_priorities
+                                .into_iter()
+                                .map(|(each_bucket, _priority)| {
+                                    (each_bucket, BroadcastPeerPriority::Primary)
+                                })
+                                .collect()
+                        } else {
+                            buckets_with_priorities
+                        }
                     };
```

### Change 4: Extend `execute_broadcast` return value and change its signature

```diff
@@ -621,19 +624,26 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf

-    pub async fn execute_broadcast<TransactionValidator: TransactionValidation>(
+    pub async fn execute_broadcast<TransactionValidator: TransactionValidation + 'static>(
         &self,
         peer: PeerNetworkId,
         scheduled_backoff: bool,
         smp: &mut SharedMempool<NetworkClient, TransactionValidator>,
-    ) -> Result<(), BroadcastError> {
+        do_check_peer_prioritized_oinori: bool,
+    ) -> Result<(String, usize, MempoolMessageId, Duration), BroadcastError> {
         let start_time = Instant::now();
-        let (message_id, transactions, metric_label) =
-            self.determine_broadcast_batch(peer, scheduled_backoff, smp)?;
+        let (message_id, transactions, metric_label) = self.determine_broadcast_batch(
```

```diff
@@ -641,6 +651,12 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf

         let latency = start_time.elapsed();
+
+        let tx_hash = transactions
+            .first()
+            .map(|tx| tx.0.committed_hash().to_hex_literal())
+            .unwrap_or("".to_string());
+
         // ... metrics logging ...
-        Ok(())
+        Ok((tx_hash, num_txns, message_id, latency))
     }
```

---

## `mempool/src/shared_mempool/tasks.rs`

### Change 1: Skip VM validation, including import changes

```diff
@@ -16,7 +16,6 @@ use crate::{
         },
         use_case_history::UseCaseHistory,
     },
-    thread_pool::IO_POOL,
     QuorumStoreRequest, QuorumStoreResponse, SubmissionStatus,
 };
@@ -28,16 +27,14 @@ use aptos_mempool_notifications::CommittedTransaction;
 use aptos_metrics_core::HistogramTimer;
 use aptos_network::application::interface::NetworkClientInterface;
-use aptos_storage_interface::state_view::LatestDbStateCheckpointView;
 use aptos_types::{
     mempool_status::{MempoolStatus, MempoolStatusCode},
     on_chain_config::{OnChainConfigPayload, OnChainConfigProvider, OnChainConsensusConfig},
     transaction::SignedTransaction,
     vm_status::{DiscardedVMStatus, StatusCode},
 };
-use aptos_vm_validator::vm_validator::{get_account_sequence_number, TransactionValidation};
+use aptos_vm_validator::vm_validator::TransactionValidation;
 use futures::{channel::oneshot, stream::FuturesUnordered};
-use rayon::prelude::*;

+// oinori: for skipping VM validation
+use aptos_types::transaction::VMValidatorResult;
```

### Change 2: Fix the broadcast interval

```diff
@@ -94,11 +94,8 @@ pub(crate) async fn execute_broadcast<NetworkClient, TransactionValidator>(
     let schedule_backoff = network_interface.is_backoff_mode(&peer);

-    let interval_ms = if schedule_backoff {
-        smp.config.shared_mempool_backoff_interval_ms
-    } else {
-        smp.config.shared_mempool_tick_interval_ms
-    };
+    // oinori: fixed 30 sec interval
+    let interval_ms = 30_000;
```

### Change 3: Skip DB state reads and sequence-number validation

```diff
@@ -285,58 +284,12 @@ where
 {
     let mut statuses = vec![];

-    let start_storage_read = Instant::now();
-    let state_view = smp
-        .db
-        .latest_state_checkpoint_view()
-        .expect("Failed to get latest state checkpoint view.");
-
-    let seq_numbers = IO_POOL.install(|| {
-        transactions
-            .par_iter()
-            .map(|(t, _, _)| {
-                get_account_sequence_number(&state_view, t.sender()).map_err(|e| { ... })
-            })
-            .collect::<Vec<_>>()
-    });
-
+    // oinori: skip DB state fetching and seq number validation for performance
     let transactions: Vec<_> = transactions
         .into_iter()
-        .enumerate()
-        .filter_map(|(idx, (t, ready_time_at_sender, priority))| {
-            if let Ok(sequence_num) = seq_numbers[idx] {
-                if t.sequence_number() >= sequence_num {
-                    return Some((t, sequence_num, ready_time_at_sender, priority));
-                } else { ... }
-            } else { ... }
-            None
+        .map(|(t, ready_time_at_sender, priority)| {
+            let sequence_num = t.sequence_number();
+            (t, sequence_num, ready_time_at_sender, priority)
         })
         .collect();
```

### Change 4: Skip VM validation in `validate_and_add_transactions`

```diff
@@ -369,23 +322,16 @@ fn validate_and_add_transactions<NetworkClient, TransactionValidator>(
 {
-    let vm_validation_timer = counters::PROCESS_TXN_BREAKDOWN_LATENCY
-        .with_label_values(&[counters::VM_VALIDATION_LABEL])
-        .start_timer();
+    // oinori: skip VM validation for performance, just pre-compute hash and length
     let validation_results = transactions
-        .par_iter()
+        .iter()
         .map(|t| {
-            let result = smp.validator.read().validate_transaction(t.0.clone());
-            if result.is_ok() {
-                t.0.committed_hash();
-                t.0.txn_bytes_len();
-            }
-            result
+            t.0.committed_hash();
+            t.0.txn_bytes_len();
+            Ok(VMValidatorResult::new(None, t.0.gas_unit_price()))
         })
-        .collect::<Vec<_>>();
-    vm_validation_timer.stop_and_record();
+        .collect::<Vec<Result<_>>>();
```

---

## `mempool/src/shared_mempool/types.rs`

### Change: Add a broadcast peer ID list to `MempoolClientRequest::SubmitTransaction`

```diff
@@ -235,7 +235,11 @@ pub type SubmissionStatusBundle = (SignedTransaction, SubmissionStatus);

 pub enum MempoolClientRequest {
-    SubmitTransaction(SignedTransaction, oneshot::Sender<Result<SubmissionStatus>>),
+    SubmitTransaction(
+        SignedTransaction,
+        oneshot::Sender<Result<SubmissionStatus>>,
+        Option<Vec<PeerNetworkId>>, // oinori: broadcast peer IDs
+    ),
     GetTransactionByHash(HashValue, oneshot::Sender<Option<SignedTransaction>>),
 }
```
