# oinori-mev lib.rs / entry point

The entry point of the `oinori-mev` crate. It handles startup, initialization, and orchestration of the entire MEV engine.

---

## `oinori/oinori-mev/src/lib.rs`
- **Purpose**: Runtime startup of the MEV engine and connection to the transaction stream

### `start_oinori_mev()`
- **Purpose**: Create a multi-threaded Tokio runtime and start the MEV engine
- **Logic**: Build a dedicated runtime with `tokio::runtime::Builder` (with LIFO slot disabled) and run `mev_transaction_handler()` in blocking mode

### `mev_transaction_handler()`
- **Purpose**: Main handler that starts and connects all MEV engine components
- **Logic**:
  1. Load environment settings from `OinoriEnvs`
  2. Create an MPSC channel for communication from the transaction stream to the MEV engine
  3. Start three asynchronous tasks in parallel:
     - `setup_and_keep_sync_engine`: engine initialization and blockchain sync maintenance
     - `start_mev_engine`: receive transactions and run MEV logic
     - `start_tx_stream`: continuously fetch transaction output from the network
  4. In the main loop, convert received stream data into `UserTransactionLog` with `MoveConverter`

---

## `oinori/oinori-mev/src/envs/mod.rs`
- **Purpose**: MEV engine configuration management via environment variables

### `OinoriEnvs` struct
| Environment variable | Field | Default value |
|---|---|---|
| `OINORI_ACCOUNT_SET_ID` | `oinori_sender_set_id` | `"default"` |
| `OINORI_RPC_URL` | `rpc_url` | `"https://fullnode.mainnet.aptoslabs.com"` |
| `OINORI_METRICS_URL` | `self_metrics_url` | `"http://localhost:9023"` |
| `OINORI_NODE_ID` | `node_id` | `"default_node"` |
| `OINORI_CONTRACT_DIR` | `contract_dir` | `"{CARGO_MANIFEST_DIR}/contracts"` |
| `OINORI_MEV_DISABLED` | `oinori_mev_disable` | `false` |
| `OINORI_MEV_DRY_RUN` | `oinori_mev_dry_run` | `false` |

---

## `oinori/oinori-mev/src/path.rs`
- **Purpose**: Compile-time embedding of project resources such as `coin.toml` via the `include_dir!` macro

---

## `oinori/oinori-mev/src/hasher/mod.rs`
- **Purpose**: Re-export of `FxHashMap` / `FxHashSet`. Uses a hash implementation optimized for performance rather than cryptographic security in internal data structures

---

## `oinori/oinori-mev/src/resource.rs`
- **Purpose**: Conversion helpers between Move's type system and Rust. Provides macros and functions for handling Move resources as JSON
- **Main macros**: `move_types_vec_to_struct!`, `get_value_from_json_obj!`, `get_value_from_struct_with_type!`
