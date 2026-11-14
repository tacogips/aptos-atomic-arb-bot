# oinori-mev engine module

The core of the MEV engine. It manages the entire pipeline from transaction monitoring to opportunity detection, route optimization, and trade execution.

---

## `oinori/oinori-mev/src/engine/mod.rs`
- **Purpose**: Main structure of the MEV engine and the transaction processing loop

### `Engine` struct
Holds the engine state:
- Sender information (private key and sequence number)
- APT balance
- List of available swaps
- Transaction-based payloads (with or without flash loans)
- Recent MEV cache

### `start_mev_engine()`
- **Purpose**: Main loop that receives user transactions from an MPSC channel and runs `run_mev()` for each transaction
- **Logic**: Prevents duplicate processing by tracking processed versions with `AtomicU64`

---

## `oinori/oinori-mev/src/engine/mev_runner/mod.rs`
- **Purpose**: Main pipeline for MEV opportunity detection and execution

### `run_mev()`
- **Purpose**: Detects and executes arbitrage opportunities from individual user transactions
- **Logic**:
  1. **Pool update**: Reflect swap changes from the user TX with `swap_pool.update_pool_with_resources()`
  2. **Filter generation**: Determine search conditions from affected swap pairs with `route_filter_by_affected_resource()`
  3. **Thala exclusion**: If Thala is not involved, exclude it from the search targets to avoid unnecessary searches
  4. **Route search**: Search for profitable routes with `search_route()`
     - `threshold_profit = 0.001 APT`
     - `loan_threshold = 90 APT`
     - `input_amounts = [200, 6, 0.2] APT` (eager) or `[6, 0.2] APT`
  5. **Execution**: Send the transaction with `fire_found_route_contract()`

### `fire_found_route_contract()`
- **Purpose**: Safety checks and execution for discovered routes
- **Logic**:
  1. **Cache check**: Verify whether the same route (input amount, output amount, and used contract) exists in the cache
  2. **Rate limiting**: Skip multiple contract calls within 1 second
  3. **Balance guard**: Skip if the account balance is < 9 APT to prevent fund loss
  4. **Execution**: Send the transaction with `call_contacat()`
  5. **Cache update**: Record the executed route in the cache

---

## `oinori/oinori-mev/src/engine/find_route.rs`
- **Purpose**: Arbitrage route discovery and profit verification

### `search_route()`
- **Purpose**: Executes the full flow of route search -> loan strategy -> amount optimization -> profit calculation
- **Logic**:

  **1. Route search**:
  Run `find_best_route()` with a 15 ms timeout and a 3-step limit

  **2. Loan strategy analysis**:
  Analyze whether the route contains a flash-loan pair, and select the loan coin strategically:
  - No USDC pair -> up to 32,000 APT (force USDC loan)
  - No USDT pair -> up to 2,000 APT
  - No BTC pair -> up to 1,936 APT
  - No APTOGE pair -> up to 1,880 APT

  **3. Amount optimization**:
  Search for the optimal input amount with `optimize_route_amount_with_loan_fee()`:
  - Step size: 0.3 APT
  - Maximum steps: 500 (up to 150 APT)

  **4. Profit calculation**:
  ```
  profit = return_amount - input_amount
  return_amount = output - min(output, loan_fee + gas_fee)
  loan_fee = flash loan fee (when input > threshold)
  gas_fee = gas_unit_price × estimated_max_gas
  ```

  **5. Filtering**:
  - `output ≤ input + loan_fee` -> reject as no profit
  - `profit < 0.001 APT` -> reject as below threshold
  - `input > 5000 APT` -> reject as exceeding the maximum constraint

  **6. Gas unit price calculation**:
  - Allocate 23.5% of gross profit to gas (large trades)
  - Reduce by 30% when Thala is involved
  - Apply an overall 40% reduction factor
  - Upper limit: 0.000011 APT/gas unit
  - Range: [100, 642.03] octas

---

## `oinori/oinori-mev/src/engine/call_order_contract.rs`
- **Purpose**: Mempool submission of signed transactions and confirmation tracking

### `call_contacat()`
- **Purpose**: Sign the transaction and broadcast it to selected peers
- **Logic**:
  1. Acquire a lock to prevent concurrent contract calls
  2. Randomly select two peers
  3. Build a signed transaction (gas price, sequence number, 1-second timeout)
  4. Send it to the mempool via `MempoolClientRequest`
  5. Start a confirmation monitoring task asynchronously

### `start_check_transaction()`
- **Purpose**: Monitor transaction confirmation and update state
- **Logic**:
  - Poll the DB every 500 ms and search for the transaction by hash
  - If found: increment the sequence number and release the lock
  - On timeout (1 second): fetch the latest sequence number from RPC and resync
  - The lock prevents the next contract call until the transaction is settled

---

## `oinori/oinori-mev/src/engine/gas_calculation.rs`
- **Purpose**: Gas estimation based on route composition

### `estimate_max_gas()`
- **Purpose**: Estimate the total gas required for a route
- **Logic**:
  1. Search `ROUTE_TO_GAS_MAP` for the gas cost of the exact route contract combination
  2. If not found, estimate based on route length:
     - Base: 60 gas (2 steps), 92 gas (3 steps or more)
     - Thala pair: add 100-700 gas depending on the coin pair
     - Flash loan: add 5 gas

---

## `oinori/oinori-mev/src/engine/cache.rs`
- **Purpose**: Cache that prevents duplicate execution of the same route within a short period
- **TTL**: 1200 seconds (20 minutes)
- **Key**: `(input amount, output amount, ordered list of used contracts)`

---

## `oinori/oinori-mev/src/engine/setup_and_keep_sync.rs`
- **Purpose**: Engine initialization and blockchain sync maintenance

### `start_up_engine()`
- **Purpose**: Initial bootstrap of the engine
- **Logic**:
  1. Wait for synchronization to complete (`synced_version + 50000 >= remote_version`)
  2. Obtain the account sequence number from RPC
  3. Load the swap pool from the DB
  4. Set the bootstrap completion flag

### `keep_sync_data_diffs()`
- **Purpose**: Periodic synchronization every 40 seconds
- **Logic**:
  1. Synchronize the sequence number from RPC (applied only when more than 30 seconds have passed since the last MEV execution)
  2. Synchronize the APT balance from the DB
  3. **Stall detection**: If transaction reception stops for 5 minutes or more, panic to force a restart

---

## `oinori/oinori-mev/src/engine/sync_engine.rs`
- **Purpose**: Check the node's blockchain synchronization status
- **Logic**: Compare the local sync version and the remote version with a 50,000-block buffer

---

## `oinori/oinori-mev/src/engine/sender_set.rs`
- **Purpose**: Load transaction sender credentials from a TOML file
