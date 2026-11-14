# oinori-mev transaction_stream module

Continuously fetches transaction outputs from the network and streams them to the MEV engine. Includes backpressure control.

---

## `oinori/oinori-mev/src/transaction_stream/mod.rs`
- **Purpose**: Main loop for the transaction stream

### `start_tx_stream()`
- **Purpose**: Main stream loop that continuously fetches transaction outputs from the network
- **Logic**:
  1. Create a tokio interval for periodic polling and a notify channel for immediate fetches
  2. Wait on both triggers in the main loop:
     - Timer tick: periodic fetch attempt
     - Notify signal: immediate fetch attempt
  3. For each fetch:
     - Confirm bootstrap synchronization is complete
     - Create a request sender (returns `None` when the max concurrent request count is exceeded)
     - Run the fetch in an asynchronous task
     - Calculate fetch size (deterministic or random range)
     - Execute `get_transaction_outputs_with_proof`
     - Send the result through an MPSC channel
     - Notify the next fetch

- **Timeout**: 45ms per request (`CLIENT_TIMEOUT_MS`)

---

## `oinori/oinori-mev/src/transaction_stream/stream_status.rs`
- **Purpose**: Stream state management and backpressure control

### `TransactionOutputStreamStatus` structure
- `latest_fetched_version`: Latest fetched version
- `flying_request_num`: Current number of in-flight requests

### Backpressure algorithm

#### `new_stream_request_sender()`
- **Purpose**: Create a request sender with backpressure control
- **Logic**:
  1. Check `flying_request_num > max_flying_request_num`
  2. If the limit is exceeded: return `None` (apply backpressure)
  3. Within the limit: create the sender

#### `TransactionOutputStreamRequestSender::Drop`
- **Purpose**: Clean up state when a request completes
- **Logic**:
  - Update `latest_fetched_version` (only if the version is newer)
  - Decrement `flying_request_num`

### `FetchTransactionRequest::as_param_versions()`
- **Purpose**: Calculate fetch parameters
- **Logic**:
  ```
  start_version = latest_fetched_version + 1
  end_version = start_version + additional_tx_fetch_size
  epoch_version = end_version
  ```
