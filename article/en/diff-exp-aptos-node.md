# Changes to the `aptos-node` package

Changes to the Aptos node startup sequence to integrate the oinori-mev runtime. The MEV engine is started automatically when the node boots.

---

## `aptos-node/.taplo.toml` (new file)
- **Purpose**: TOML formatter settings specific to the aptos-node directory
- **Contents**: Exclude Cargo.lock, set indentation to 4 spaces, and set column width to 300 characters for long dependency lines in Cargo.toml

```diff
+#include = []
+exclude = ["**/Cargo.lock"]
+
+#https://taplo.tamasfe.dev/configuration/formatter-options.html
+[formatting]
+indent_string = "    "
+column_width = 300
```

---

## `aptos-node/Cargo.toml`

### Change: added the oinori-mev dependency
- **Change**: Added `oinori-mev = { path = "../oinori/oinori-mev" }`
- **Purpose**: Add the oinori-mev crate as a dependency of the aptos-node binary. This makes it possible to start the MEV engine from inside the node binary

```diff
@@ -78,6 +78,10 @@ serde_yaml = { workspace = true }
 tokio = { workspace = true }
 url = { workspace = true }


+## TODO add  oinori-mev
+oinori-mev = { path = "../oinori/oinori-mev" }
+
 [target.'cfg(unix)'.dependencies]
 jemallocator = { workspace = true }
```

---

## `aptos-node/src/lib.rs`

### Change 1: added an oinori runtime field to `AptosHandle`
- **Change**: Added the `_oinori_mev_runtime: Option<Runtime>` field
- **Purpose**: Bind the lifetime of the oinori-mev async runtime to the Aptos node handle so that the MEV engine keeps running while the node is running

```diff
@@ -207,6 +207,7 @@ pub struct AptosHandle {
     _state_sync_runtimes: StateSyncRuntimes,
     _telemetry_runtime: Option<Runtime>,
     _indexer_db_runtime: Option<Runtime>,
+    _oinori_mev_runtime: Option<Runtime>,
 }
```

### Change 2: cloned `aptos_data_client`
- **Change**: `aptos_data_client` -> `aptos_data_client.clone()` when passing it to `inspection_service`
- **Purpose**: `aptos_data_client` also needs to be passed to oinori-mev, so it is cloned instead of moved

```diff
@@ -675,7 +677,7 @@ pub fn setup_environment_and_start_node(
     // Start the node inspection service
     services::start_node_inspection_service(
         &node_config,
-        aptos_data_client,
+        aptos_data_client.clone(),
         peers_and_metadata.clone(),
     );
```

### Change 3: obtained `mempool_client_sender`
- **Change**: Added `mempool_client_sender` to the return value of `bootstrap_api_and_indexer`
- **Purpose**: Obtain the Sender channel used by the MEV engine to send transactions directly to the mempool

```diff
@@ -687,6 +689,7 @@ pub fn setup_environment_and_start_node(
         indexer_runtime,
         indexer_grpc_runtime,
         internal_indexer_db_runtime,
+        mempool_client_sender, // oinori: used for mev tx submission
     ) = services::bootstrap_api_and_indexer(&node_config, db_rw.clone(), chain_id, indexer_db_opt)?;
```

### Change 4: started the oinori-mev runtime after state sync completed
- **Change**: Call `oinori_mev::start_oinori_mev()` after `state_sync_runtimes.block_until_initialized()`
- **Purpose**: Start the MEV engine after state sync has completed, that is, after the latest chain state has been reflected in the local DB
- **Arguments**:
  - `chain_id`: chain ID, used to identify mainnet
  - `aptos_data_client`: client for data communication with peers
  - `db_rw.reader`: read interface for the local DB
  - `mempool_client_sender`: transaction submission channel to the mempool

```diff
@@ -713,11 +716,21 @@ pub fn setup_environment_and_start_node(
         &vtxn_pool,
     );

+    println!("======== oinori start sync...");
     // Wait until state sync has been initialized
     debug!("Waiting until state sync is initialized!");
     state_sync_runtimes.block_until_initialized();
     debug!("State sync initialization complete.");

+    println!("======== oinori finish sync...");
+    // Start oinori MEV runtime
+    let oinori_mev_runtime = oinori_mev::start_oinori_mev(
+        chain_id,
+        aptos_data_client,
+        Arc::clone(&db_rw.reader),
+        mempool_client_sender,
+    )?;
+
     // Create the consensus observer and publisher (if enabled)
```

### Change 5: added the oinori runtime to `AptosHandle` initialization

```diff
@@ -759,7 +772,9 @@ pub fn setup_environment_and_start_node(
         _peer_monitoring_service_runtime: peer_monitoring_service_runtime,
         _state_sync_runtimes: state_sync_runtimes,
         _telemetry_runtime: telemetry_runtime,
+
         _indexer_db_runtime: internal_indexer_db_runtime,
+        _oinori_mev_runtime: Some(oinori_mev_runtime),
     })
 }
```

---

## `aptos-node/src/services.rs`

### Change 1: skipped indexer startup
- **Change**: Removed the calls to `bootstrap_indexer_grpc` and `indexer::bootstrap_indexer`, and returned `None` instead
- **Purpose**: Improve performance by skipping the indexer-related runtime. The indexer is not needed for an MEV node, so it is skipped to save resources

### Change 2: added `mempool_client_sender` to the return value
- **Change**: Added `Sender<MempoolClientRequest>` to the function's return tuple
- **Purpose**: Return the sender side of the mempool client channel to the caller (`lib.rs`) so that oinori-mev can submit transactions through the mempool

```diff
@@ -53,6 +53,7 @@ pub fn bootstrap_api_and_indexer(
     Option<Runtime>,
     Option<Runtime>,
     Option<Runtime>,
+    Sender<MempoolClientRequest>,
 )> {
     // Create the mempool client and sender
     let (mempool_client_sender, mempool_client_receiver) =
@@ -94,30 +95,16 @@ pub fn bootstrap_api_and_indexer(
         None
     };

-    // Creates the indexer grpc runtime
-    let indexer_grpc = bootstrap_indexer_grpc(
-        node_config,
-        chain_id,
-        db_rw.reader.clone(),
-        mempool_client_sender.clone(),
-        indexer_reader,
-    );
-
-    // Create the indexer runtime
-    let indexer_runtime = indexer::bootstrap_indexer(
-        node_config,
-        chain_id,
-        db_rw.reader.clone(),
-        mempool_client_sender,
-    )?;
+    // oinori: skip indexer bootstrap for performance

     Ok((
         mempool_client_receiver,
         api_runtime,
-        indexer_table_info_runtime,
-        indexer_runtime,
-        indexer_grpc,
+        None, // indexer_table_info_runtime
+        None, // indexer_runtime
+        None, // indexer_grpc
         db_indexer_runtime,
+        mempool_client_sender,
     ))
 }
```
