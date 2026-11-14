# mempool パッケージの変更

mempoolはブロックチェーンのトランザクションプールであり、ブロードキャストとバリデーションを管理する。oinoriではMEVトランザクションの高速処理のため、検証ロジックの簡略化と即時ブロードキャスト機能を追加している。

---

## `mempool/Cargo.toml`

### 変更箇所: chrono依存関係の追加
- **変更内容**: `chrono = { workspace = true }` を追加
- **目的**: タイムスタンプ処理のため（ブロードキャストタイミングの管理等）

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

### 変更箇所1: 重複トランザクションチェックの削除
- **変更内容**: `insert`メソッド内の重複トランザクション検出ロジック（約50行）を削除
- **目的**: MEVトランザクションの高速投入のため
- **背景**: 通常のmempoolでは、同一アカウント・同一シーケンス番号のトランザクションが既にある場合、ガス価格が高い場合のみ更新を許可する（ガスバンピング）。oinoriでは自分のMEVトランザクションの投入速度が最優先であるため、この検証をスキップする

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

### 変更箇所2: `check_is_full_after_eviction`メソッドおよび`check_txn_ready`メソッドの削除
- **変更内容**: 両メソッド全体を削除（合計約70行）
- **目的**: mempoolフル時のEviction処理とトランザクション準備完了チェックをスキップ

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

### 変更箇所1: `MempoolClientRequest::SubmitTransaction`のパターンマッチ変更
- **変更内容**: 第3引数 `broadcast_peer_ids` を受け取るように変更

### 変更箇所2: トランザクション送信後の即時ブロードキャスト
- **変更内容**: `process_client_transaction_submission`の後に`.then()`で即時ブロードキャスト処理を連鎖
- **目的**: MEVトランザクションを特定のバリデータピアに即座にブロードキャストする

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

### 変更箇所1: ACKのリトライ・バックオフ処理のスキップ

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

### 変更箇所2: `determine_broadcast_batch`に`do_check_peer_prioritized_oinori`パラメータ追加

```diff
@@ -358,6 +349,7 @@ impl<NetworkClient: NetworkClientInterface<MempoolSyncMsg>> MempoolNetworkInterf
         peer: PeerNetworkId,
         scheduled_backoff: bool,
         smp: &mut SharedMempool<NetworkClient, TransactionValidator>,
+        do_check_peer_prioritized_oinori: bool,
     ) -> Result<
```

### 変更箇所3: oinoriブロードキャスト時のプライオリティ強制

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

### 変更箇所4: `execute_broadcast`の戻り値拡張とシグネチャ変更

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
+            peer,
+            scheduled_backoff,
+            smp,
+            do_check_peer_prioritized_oinori,
+        )?;
+
         let num_txns = transactions.len();
         let send_time = SystemTime::now();
-        self.send_batch_to_peer(peer, message_id.clone(), transactions)
+
+        self.send_batch_to_peer(peer, message_id.clone(), transactions.clone())
             .await?;
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

### 変更箇所1: VM検証のスキップ（import変更含む）

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

### 変更箇所2: ブロードキャスト間隔の固定化

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

### 変更箇所3: DB状態読み取り・シーケンス番号検証のスキップ

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

### 変更箇所4: `validate_and_add_transactions`のVM検証スキップ

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

### 変更箇所: `MempoolClientRequest::SubmitTransaction`にブロードキャストピアIDリスト追加

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
