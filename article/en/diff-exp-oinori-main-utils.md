# oinori-mev Utility Modules

Helper modules for account management, address conversion, balance differences, metrics, validator information, and more.

---

## `oinori/oinori-mev/src/account/mod.rs`
- **Purpose**: Retrieve account information from the RPC endpoint
- `get_account_via_rpc()`: Fetches account data with the REST client

---

## `oinori/oinori-mev/src/account_address/mod.rs`
- **Purpose**: Convert addresses between hex strings and Aptos types

### `adjust_hex_address()`
- **Purpose**: Pad a hex address to 64 characters (32 bytes)
- **Logic**: Remove the `0x` prefix and left-pad with zeros to 64 characters

### `hex_to_account_address()` / `hex_to_hash_value()`
- **Purpose**: Convert a hex string into `AccountAddress` / `HashValue`

---

## `oinori/oinori-mev/src/account_address/private_key.rs`
- **Purpose**: Convert a hex-encoded private key into an `Ed25519PrivateKey`
- `hex_to_private_key()`: Remove the `0x` prefix -> decode hex -> construct `Ed25519PrivateKey`

---

## `oinori/oinori-mev/src/difference/coin_diff.rs`
- **Purpose**: Track coin balance differences across blockchain versions

### `CoinAmountDiff::from_versions()`
- **Purpose**: Calculate balance differences across multiple versions
- **Logic**:
  1. Receive a target version list `(v1, v2, v3, ...)`
  2. Load the coin balance at `v1`
  3. For each subsequent version `v(i)`:
     - Load the coin balance at `v(i)`
     - Compare it with `v(i-1)` and create a diff
     - Set `v(i)` as the next `before`
  4. Collect and return all diffs

---

## `oinori/oinori-mev/src/metrics/mod.rs`
- **Purpose**: Extract sync progress from the Prometheus metrics endpoint

### `get_metrics()`
- **Purpose**: Obtain the synced version from metrics
- **Logic**:
  1. Fetch metrics from `{metrics_host}/metrics`
  2. Split by newline
  3. Extract the version with the regular expression `aptos_state_sync_version.type="synced". ([0-9]+)`

---

## `oinori/oinori-mev/src/validator/mod.rs`
- **Purpose**: Retrieve validator network information and geolocation

### `fetch_validators()`
- **Purpose**: Fetch the validator set from RPC
- **Logic**:
  1. Retrieve the `0x1::stake::ValidatorSet` resource via RPC
  2. From each validator entry:
     - Extract the account address (`peer_id`)
     - Parse the validator network address
     - Obtain consensus voting power
  3. Return the results as an `OinoriValidator` list

### `insert_validator_countries()`
- **Purpose**: Attach geolocation information to validators
- **Logic**:
  1. Obtain each validator's IP/DNS address
     - Prefer `find_ip_addr()`, otherwise fall back to `find_dns()`
  2. Query `http://ip-api.com/json/{address}`
  3. Set the country name, timezone, and ISP
  4. Sleep for 1 second between requests to respect rate limits
