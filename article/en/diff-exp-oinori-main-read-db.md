# oinori-mev read_db module

An interface for reading state from the local Aptos DB. It provides access to resources, transactions, and ledger information.

---

## `oinori/oinori-mev/src/read_db/state.rs`
- **Purpose**: DB reads for account resources

### `get_resources_of_account()`
- **Purpose**: Retrieve all resources for an account at a specified version
- **Logic**:
  1. Convert the hex address to `AccountAddress`
  2. Determine the target version (explicitly specified or latest in the DB)
  3. Create the address `StateKeyPrefix`
  4. Scan state values with the DB prefix iterator:
     - Filter only `Resource` and `ResourceGroup` paths
     - Skip `Code` paths
  5. Convert raw state values to `MoveResource` with `MoveConverter`
  6. Return them together with the version

---

## `oinori/oinori-mev/src/read_db/tx.rs`
- **Purpose**: DB reads for transactions

### `get_tx_by_hash()`
- **Purpose**: Search for transactions by hash
- **Use case**: Used by `start_check_transaction()` for transaction confirmation monitoring

### `get_tx_outputs()`
- **Purpose**: Batch retrieval of transaction outputs
- **Constraint**: `MAX_TX_OUTPUT_LIMIT = 10000`

---

## `oinori/oinori-mev/src/read_db/ledger.rs`
- **Purpose**: Retrieve the latest ledger information
- **Logic**: Extract the V0 variant with `DbReader::get_latest_ledger_info()`

---

## `oinori/oinori-mev/src/ledger/rpc.rs`
- **Purpose**: Retrieve ledger information from the RPC endpoint
- **Logic**: Call `get_ledger_information()` with the REST client and return `State` (version and chain information)
