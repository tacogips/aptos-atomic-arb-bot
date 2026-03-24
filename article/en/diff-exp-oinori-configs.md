# Changes in the `oinori-configs` Directory (Newly Added)

Configuration files for running as an Aptos mainnet fullnode. Initial mainnet settings for the oinori-mev node to connect to.

---

## `oinori-configs/mainnet-config/fullnode.yaml` (newly added)
- **Purpose**: fullnode configuration for the oinori node
- **Key settings**:
  - **base**: data directory (`/opt/aptos/data`), role (`full_node`), waypoint file path
  - **execution**: location of `genesis.blob`
  - **api**: REST API enabled (listens on `0.0.0.0:8080`)
  - **full_node_networks**: connects to the public network, `onchain` discovery, port 6182
  - **state_sync**: `DownloadLatestStates` bootstrapping, `ApplyTransactionOutputs` continuous syncing

```yaml
+base:
+  data_dir: "/opt/aptos/data"
+  role: "full_node"
+  waypoint:
+    from_file: "/opt/aptos/genesis/waypoint.txt"
+
+execution:
+  genesis_file_location: "/opt/aptos/genesis/genesis.blob"
+
+api:
+  enabled: true
+  address: "0.0.0.0:8080"
+
+full_node_networks:
+  - network_id: "public"
+    discovery_method: "onchain"
+    listen_address: "/ip4/0.0.0.0/tcp/6182"
+
+inspection_service:
+  address: "0.0.0.0"
+  port: 9023
+  expose_configuration: false
+  expose_system_information: true
+
+state_sync:
+  state_sync_driver:
+    bootstrapping_mode: DownloadLatestStates
+    continuous_syncing_mode: ApplyTransactionOutputs
+    num_versions_to_skip_snapshot_sync: 1000000000
+    progress_check_interval_ms: 5
+    max_pending_data_chunks: 100
+
+  data_streaming_service:
+    global_summary_refresh_interval_ms: 5
+    max_concurrent_requests: 12
+    max_concurrent_state_requests: 12
+    max_data_stream_channel_sizes: 100
+
+  aptos_data_client:
+    max_state_chunk_size: 8000
+    max_transaction_chunk_size: 6000
+    max_transaction_output_chunk_size: 3000
```

Note: the latter half of the file contains many commented-out experimental tuning values (mempool settings, `storage_service`, `aptos_data_client`, etc.), which are retained as reference values for production use.

---

## `oinori-configs/mainnet-config/fullnode-for-official-debug.yaml` (newly added)
- **Purpose**: minimal configuration for debugging against the official node
- **Contents**: only `base`, `execution`, `full_node_networks`, and `api` are configured. All detailed parameters are commented out

```yaml
+base:
+  data_dir: "/opt/aptos/data"
+  role: "full_node"
+  waypoint:
+    from_file: "/opt/aptos/genesis/waypoint.txt"
+
+execution:
+  genesis_file_location: "/opt/aptos/genesis/genesis.blob"
+
+full_node_networks:
+  - network_id: "public"
+    discovery_method: "onchain"
+    listen_address: "/ip4/0.0.0.0/tcp/6182"
+
+api:
+  enabled: true
+  address: "0.0.0.0:8080"
```

---

## `oinori-configs/mainnet-config/genesis.blob` (newly added, binary)
- **Purpose**: Aptos mainnet initial state (genesis) file
- **Contents**: binary file containing the mainnet genesis block state (about 996 KB)
- **Background**: when a fullnode starts for the first time, it needs to know the chain's initial state. This is identical to the file distributed by Aptos official sources

---

## `oinori-configs/mainnet-config/waypoint.txt` (newly added)
- **Purpose**: initial waypoint (checkpoint) for mainnet
- **Background**: the hash of the genesis block (height 0). Used to verify the chain's validity when the node starts

```
+0:6072b68a942aace147e0655c5704beaa255c84a7829baa4e72a500f1516584c4
```
