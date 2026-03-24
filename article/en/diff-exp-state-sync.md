# Changes to the `state-sync` Package

Changes to the data client and streaming service related to state sync. This includes simplifying the peer selection logic and adding a peer selection method for MEV transaction submission.

---

## `state-sync/aptos-data-client/src/client.rs`

### Change 1: Simplify the peer count calculation logic for multi-fetch
- **Change**: Removed roughly 30 lines of peer count calculation logic based on the multi-fetch configuration, and fixed `num_peers_for_request = 2`
- **Purpose**: Reduce overhead in peer selection

```diff
@@ -276,37 +275,8 @@ impl AptosDataClient {
         }

         // Otherwise, determine the number of peers to select for the request
-        let multi_fetch_config = self.data_client_config.data_multi_fetch_config;
-        let num_peers_for_request = if multi_fetch_config.enable_multi_fetch {
-            // Calculate the total number of priority serviceable peers
-            let mut num_serviceable_peers = 0;
-            for (index, peers) in serviceable_peers_by_priorities.iter().enumerate() {
-                if (num_serviceable_peers == 0)
-                    || (index < serviceable_peers_by_priorities.len() - 1)
-                {
-                    num_serviceable_peers += peers.len();
-                }
-            }
-
-            let peer_ratio_for_request =
-                num_serviceable_peers / multi_fetch_config.multi_fetch_peer_bucket_size;
-            let mut num_peers_for_request = multi_fetch_config.min_peers_for_multi_fetch
-                + (peer_ratio_for_request * multi_fetch_config.additional_requests_per_peer_bucket);
-
-            num_peers_for_request = min(num_peers_for_request, num_serviceable_peers);
-
-            min(
-                num_peers_for_request,
-                multi_fetch_config.max_peers_for_multi_fetch,
-            )
-        } else {
-            1 // Multi-fetch is disabled (only select a single peer)
-        };
+        // oinori: fixed peer count
+        let num_peers_for_request = 2;
 ```

### Change 2: Switch peer selection from latency-based to random
- **Change**: `self.choose_random_peers_by_latency` -> `utils::choose_random_peers`
- **Purpose**: Change peer selection during state sync from latency-weighted random to simple random

```diff
@@ -383,9 +353,9 @@ impl AptosDataClient {
         // Select peers by priority (starting with the highest priority first)
         let mut selected_peers = HashSet::new();
         for serviceable_peers in serviceable_peers_by_priorities {
-            // Select peers by distance and latency
+            // oinori: choose randomly instead of weighted by latency
             let num_peers_remaining = num_peers_for_request.saturating_sub(selected_peers.len());
-            let peers = self.choose_random_peers_by_latency(serviceable_peers, num_peers_remaining);
+            let peers = utils::choose_random_peers(num_peers_remaining, serviceable_peers);

             selected_peers.extend(peers);
 ```

### Change 3: Add a peer selection method for MEV transaction submission
- **New method**: `choose_peer_for_oinori_tx_send`
- **Purpose**: Select low-latency validator peers when the MEV engine submits arbitrage transactions

```diff
@@ -522,7 +492,26 @@ impl AptosDataClient {
         utils::extend_with_random_peers(selected_peers, serviceable_peers, num_peers_to_choose)
     }

-    /// Identifies the peers with the specified priority that can service the given request
+    /// oinori: choose peers for tx send by latency
+    pub fn choose_peer_for_oinori_tx_send(&self, num: usize) -> Result<Vec<PeerNetworkId>, Error> {
+        let (priority_peers, _regular_peers) = self.get_priority_and_regular_peers()?;
+        let serviceable_peers = if !priority_peers.is_empty() {
+            priority_peers
+        } else {
+            return Err(Error::DataIsUnavailable(
+                "no peers for oinori tx send".to_string(),
+            ));
+        };
+
+        let choose_num = std::cmp::min(num, serviceable_peers.len());
+        Ok(self
+            .choose_random_peers_by_latency(serviceable_peers, choose_num)
+            .into_iter()
+            .collect())
+    }
+
+    /// Identifies the peers in the given set of prospective peers
+    /// that can service the specified request.
     fn identify_serviceable(
 ```

### Change 4: Remove trace logs when responses are received

```diff
@@ -783,13 +772,7 @@ impl AptosDataClient {
             .await;
         match result {
             Ok(response) => {
-                trace!(
-                    (LogSchema::new(LogEntry::StorageServiceResponse)
-                        .event(LogEvent::ResponseSuccess)
-                        .request_type(&request.get_label())
-                        .request_id(id)
-                        .peer(&peer))
-                );
+                // oinori: skip trace logging for performance
 ```

---

## `state-sync/aptos-data-client/src/peer_states.rs`

### Change: Simplify the peer request servicing check
- **Change**: Removed the `storage_summary`-based request servicing check for peers and always return `true`
- **Purpose**: Treat every peer as immediately available

```diff
@@ -212,18 +212,8 @@ impl PeerStates {
             return true;
         }

-        // Check if the peer can service the request
-        if let Some(peer_state) = self.peer_to_state.get(peer) {
-            return match peer_state.get_storage_summary_if_not_ignored() {
-                Some(storage_summary) => {
-                    storage_summary.can_service(&self.data_client_config, time_service, request)
-                },
-                None => false, // The peer is temporarily ignored
-            };
-        }
-
-        // Otherwise, the request cannot be serviced
-        false
+        // oinori: all peers can service any request
+        true
     }
 ```

---

## `state-sync/data-streaming-service/src/stream_engine.rs`

### Change: Minor formatting changes
- **Change**: Only added/removed blank lines in two places
- **Purpose**: Minor code formatting adjustment. No logical change.

```diff
@@ -260,7 +260,6 @@ impl DataStreamEngine for StateStreamEngine {
                 global_data_summary.optimal_chunk_sizes.state_chunk_size,
                 self.clone().into(),
             )?;
-
             // Return the requests
             self.update_request_tracking(&client_requests)?;
             return Ok(client_requests);
@@ -1231,6 +1230,7 @@ impl DataStreamEngine for ContinuousTransactionStreamEngine {
                 optimal_chunk_sizes,
                 self.clone().into(),
             )?;
+
             self.update_request_tracking(&client_requests, &target_ledger_info)?;
 ```
