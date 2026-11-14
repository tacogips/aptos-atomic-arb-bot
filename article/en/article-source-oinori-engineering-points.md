# Technical Techniques in oinori - Implementation Strategy for the Aptos MEV Bot

oinori is an atomic arbitrage bot on Aptos. Because of the nature of MEV, **fast transaction acquisition**, **stable connection maintenance**, **fast route search**, and **efficient on-chain execution** directly affect revenue. This article explains the four main technical techniques used in oinori.

---

## Technique 1: Connection ID Rotation and a Noise Protocol Hack

### Problem: bans from validators

oinori communicates with validators using Aptos's proprietary P2P protocol. Because it is an MEV bot, **requests containing errors occur frequently** due to simulation failures and timing issues. Aptos validators have a mechanism that detects clients sending excessive error requests and blocks connections from those clients (rate limit / ban).

Ordinary fullnodes connect with a fixed identity (x25519 key pair), so once they are banned, recovery is difficult.

### Solution: periodic reconnection with random IDs

oinori's solution is to **generate a new x25519 private key at every connection and reconnect with a different PeerID**. This makes the validator see it as a different client each time.

#### Overall implementation

```
ConnectivityManager
  │
  ├── oinori_new_self_peer_id()     ← generates a random x25519 key + PeerID every time
  ├── oinori_peer_connected_log     ← records the connection time for each peer
  ├── oinori_reconnect_ticker       ← reconnection timer at 3-second intervals
  └── oinori_reconnect_peers()      ← disconnects connections older than 10 seconds → reconnects with a new ID
        │
        ▼
  ConnectionRequestSender::dial_peer(self_peer_private_key, self_peer_id, ...)
        │
        ▼
  AptosNetTransport::dial(oinori_self_peer_private_key, oinori_self_peer_id, ...)
        │
        ▼
  oinori_upgrade_outbound()          ← custom Noise handshake
        │
        ▼
  NoiseUpgrader::oinori_upgrade_outbound()  ← performs Noise IK handshake with a custom private key
```

#### Why a Noise protocol hack was necessary

Communication between Aptos nodes is encrypted with the [Noise Protocol Framework](https://noiseprotocol.org/) using the IK pattern. In the ordinary `upgrade_outbound`, the handshake is performed with the node's fixed private key (`self.noise_config`) set at startup.

Because oinori wants to change the ID for every connection, it **made the private key used during the handshake passable as an external parameter**. That is the `oinori_upgrade_outbound` method.

```rust
// Ordinary upgrade_outbound: uses the fixed self.noise_config
pub async fn upgrade_outbound(&self, socket, remote_peer_id, remote_public_key, ...)

// oinori version: accepts an arbitrary private key as a parameter
pub async fn oinori_upgrade_outbound(&self, socket,
    self_peer_private_key: x25519::PrivateKey,  // ← a different random key each time
    self_peer_id: PeerId,                        // ← PeerID derived from the key
    remote_peer_id, remote_public_key, ...)
```

Internally:
```rust
// oinori: create a new NoiseConfig from the passed private key
let self_noise_config = noise::NoiseConfig::new(self_peer_private_key);

// Run the IK handshake with this temporary NoiseConfig
let initiator_state = self_noise_config
    .initiate_connection(&mut rng, prologue_msg, remote_public_key, ...)?;
```

#### Scope of the refactor

This change spans the entire network stack in aptos-core:

| Layer | File | Change |
|---|---|---|
| Encryption | `crates/aptos-crypto/src/x25519.rs` | Added `Clone` to `PrivateKey` |
| Noise handshake | `network/framework/src/noise/handshake.rs` | Added `oinori_upgrade_outbound` method |
| Transport | `network/framework/src/transport/mod.rs` | Added `oinori_upgrade_outbound` function, changed `dial` signature |
| Transport trait | `network/netcore/src/transport/mod.rs` | Added ID argument to `Transport::dial` |
| PeerManager | `network/framework/src/peer_manager/mod.rs` | Added ID argument to `DialPeer` |
| PeerManager type | `network/framework/src/peer_manager/types.rs` | Extended `ConnectionRequest::DialPeer` type |
| Sender | `network/framework/src/peer_manager/senders.rs` | Changed `dial_peer` signature |
| NetworkSender | `network/framework/src/protocols/network/mod.rs` | Changed `dial_peer` signature |
| ConnectivityManager | `network/framework/src/connectivity_manager/mod.rs` | Added the full reconnection logic |

This is the part of the implementation where oinori put the most engineering effort, and it was also the most expensive refactor.

#### Reducing physical latency through colocation

In addition to protocol changes, oinori also reduces latency through **server placement itself**. In production, it places nodes on AWS EC2 in Europe, where there are many validators, and targets Europe validators for connection, fetching, and sending. This shortens RTT caused by physical distance.

This is a separate technique from ID rotation for ban avoidance. Its goal is simpler: **increase speed by bringing transaction acquisition and broadcast closer to a dense validator set**.

In the implementation, after `ConnectivityManager::choose_peers_to_dial()` enumerates eligible peers, `oinori_filter_sort_eligible_peers()` filters the connection targets by region:

- **Default for production**: target only Europe validators
- **Local environment**: if the environment variable `KOREAN_VALIDATOR=1` is set, switch to a target set of validators around Korea, Japan, and Singapore

It is easier to understand this switch as a way to keep local development near validators close to Japan, while in production **placing nodes in Europe, where there are more validators and the environment is competitively advantageous**.

---

## Technique 2: A Custom Transaction Stream That Bypasses DataSummary

### Problem: constraints imposed by `DataSummary` in the standard state sync

The standard Aptos state-sync pipeline works as follows:

```
DataStreamingService → StreamEngine → AptosDataClient → peers
                         ▲
                         │
                    GlobalDataSummary  ← collected periodically from peers by DataSummaryPoller
                         ▲
                         │
                    StorageServerSummary / DataSummary  ← published by each peer
```

The important structure here is **`DataSummary`** (`state-sync/storage-service/types/src/responses.rs`):

```rust
pub struct DataSummary {
    /// ledger info corresponding to the highest synced version in storage
    pub synced_ledger_info: Option<LedgerInfoWithSignatures>,
    pub epoch_ending_ledger_infos: Option<CompleteDataRange<Epoch>>,
    pub states: Option<CompleteDataRange<Version>>,
    pub transactions: Option<CompleteDataRange<Version>>,
    pub transaction_outputs: Option<CompleteDataRange<Version>>,
}
```

Each peer publishes its own `StorageServerSummary` (which includes `DataSummary`) to the network. `DataSummaryPoller` collects this periodically and aggregates it into a `GlobalDataSummary`.

`StreamEngine` (`ContinuousTransactionOutputStreamEngine`) refers to this `GlobalDataSummary` when deciding the target for a transaction stream:

```rust
// state-sync/data-streaming-service/src/stream_engine.rs:489-494
if let Some(highest_synced_ledger_info) = advertised_data.highest_synced_ledger_info() {
    let (next_request_version, _) = self.next_request_version_and_epoch;
    if next_request_version > highest_synced_ledger_info.ledger_info().version() {
        Ok(None) // no target -> do not request
    } else {
        Ok(Some(highest_synced_ledger_info))
    }
}
```

**This mechanism has a fatal limitation for MEV:**

1. **Sequential processing**: `StreamEngine` processes versions in order. It cannot skip intermediate versions and jump to the latest one
2. **Polling delay**: there is lag before `DataSummaryPoller` collects a peer's `StorageServerSummary`. Even if a peer has the latest data, no request is issued until it is reflected in `GlobalDataSummary`
3. **`AptosDataClient`-level constraints**: when sending a request, peer selection also evaluates `StorageServerSummary.can_service()`, and requests outside the peer's advertised data range have nowhere to go

**In short: in the standard pipeline, you can only request data that is "published" and "synced" on the network, and structurally you cannot fetch the latest transaction as fast as possible.**

### Solution: a custom stream that bypasses StreamEngine

oinori's `transaction_stream` module (`oinori/oinori-mev/src/transaction_stream/`) **completely bypasses** `DataStreamingService`/`StreamEngine` and calls the `AptosDataClient` API directly.

```
oinori transaction_stream
  │
  ├── TransactionOutputStreamStatus  ← custom state management
  │     latest_fetched_version        ← latest fetched version
  │     flying_request_num            ← number of concurrent requests
  │
  ├── start_tx_stream()              ← main loop (dual trigger with tick + notify)
  │
  └── AptosDataClient                ← called directly without going through StreamEngine
        .get_transaction_outputs_with_proof(epoch, start, end, 45ms)
```

#### Core idea: version management that does not depend on `DataSummary`

```rust
// oinori/oinori-mev/src/transaction_stream/stream_status.rs
fn as_param_versions(&self, latest_fetched_version: u64) -> (u64, u64, u64) {
    let start_version = latest_fetched_version + 1;
    let end_version = start_version + self.additional_get_tx_num as u64;
    let epoch_version = end_version;
    (start_version, end_version, epoch_version)
}
```

oinori determines the next version range to fetch based only on its own `latest_fetched_version`. It does not consult `GlobalDataSummary` or `DataSummary` at all. It **speculatively** requests versions `additional_get_tx_num` ahead.

- If the validator receiving the request is synced up to that version, data is returned
- If not, an error is returned -> retry immediately with a 45 ms timeout

#### How it is accelerated

1. **Dual-trigger approach**: in addition to periodic ticks, the next fetch starts immediately using the previous fetch completion as a notify signal. This enables continuous fetching without depending on the tick interval
2. **Concurrent requests**: `flying_request_num` manages the number of concurrent requests, allowing multiple fetches to run in parallel
3. **Very short timeout**: 45 ms (`CLIENT_TIMEOUT_MS`). If there is no response, immediately move on to the next request
4. **Probabilistic fetch size**: the `stochastic_additional_tx_fetch_size` option adds random variation to the fetch size. This avoids fixed-pattern detection while speculatively covering different version ranges

#### Error handling

Because the requests are speculative, errors occur frequently. oinori expects this and suppresses noisy errors:

```rust
// "cannot be satisfied" -> the peer does not have the data (expected)
// "Rpc timed out"       -> timeout (expected)
// "dropped response"    -> response dropped (expected)
if !err_msg.contains("cannot be satisfied")
    && !err_msg.contains("Rpc timed out")
    && !err_msg.contains("dropped response")
{
    println!("---- oinori stream request error ...");
}
```

---

## Technique 3: Route Filtering and Optimal Input Amount Search

### Problem: exhaustive search is temporally impossible

In atomic arbitrage, you need to find profitable routes from the state of swap pools changed by a transaction, such as a circular route like APT -> coin A -> coin B -> APT. But if you search all combinations across multiple DEXs and multiple pairs, you cannot finish within a **15 ms timeout**.

### Solution 1: route filtering based on change patterns (`route_filter`)

oinori analyzes **which swap resources changed and how they changed** in a transaction, and narrows the search space in advance.

```
transaction -> affected_swap_updates (list of swaps that changed)
  │
  ├── 1 item -> single_affect:  decide where to place it in the 3-step route from the DEX and coin increase/decrease direction
  ├── 2 items -> two_affect:    chain route detection + shortcut decision with reverse-order checks
  └── 3+ items -> multiple_affect: classify into APT appreciation / depreciation / non-APT pairs and aggregate
```

The core is looking at whether a coin **appreciated** or **depreciated**:

- **APT appreciated** (APT flowed out of the pool) -> place that DEX at the **route start** (Step 1). You can buy other coins cheaply with APT
- **APT depreciated** (APT flowed into the pool) -> place that DEX at the **route end** (Step 3). You can buy back APT cheaply with other coins
- **Non-APT pair** -> place it in the **middle step** (Step 2)

```rust
// Set filters for each step of the 3-step route
SearchRouteCondition {
    swap_filters_for_steps: Some((Some(Liquid), None, Some(Pancake))),
    //                             ↑ Step1       Step2    ↑ Step3
    coin_filters_for_steps: (first_filters, second_filters, third_filters),
    input_amounts: vec![200 APT, 6 APT, 0.2 APT],
}
```

Because changes to pairs directly involving APT are highly likely to be profitable, the search uses **eager inputs** (three patterns including 200 APT).

When two items change, it also performs **chain route detection**. It detects patterns such as "DEX1: APT -> USDC, DEX2: USDC -> APT", where the output coin of one step becomes the input coin of the next, and directly specifies Step 1 = DEX1 and Step 2 = DEX2.

Thala has complex coupling across multiple pools, so when Thala-related changes are detected, filtering is **disabled** and the search falls back to full exploration.

### Solution 2: parallel search + timeout-based best selection

`find_best_route` in `find_route.rs` searches in parallel by combining **multiple input amounts × the starting pair of each DEX**:

```
input amounts [200 APT, 6 APT, 0.2 APT] × DEX starting points [Liquid, Pancake, Cetus, ...]
  │
  ├── start each combination concurrently with `tokio::task::spawn`
  ├── recursively perform depth-first search up to `max_depth=3`
  └── aggregate discovered routes through `unbounded_channel`
        │
        ▼
   `tokio::select!` chooses the best route on a 15 ms timeout or when all tasks complete
```

Among the routes found within the timeout, the one with the highest profit is selected. It does not wait for the exhaustive search to finish.

### Solution 3: two-stage input amount optimization

After the best route is found, the **input amount is optimized** to maximize profit:

```
1. Binary search (up to 30 iterations)
   smaller = input/2, middle = input, greater = input*2
   -> narrow down the range of inputs that maximizes profit

2. Linear search (up to 500 steps × 0.3 APT increments)
   -> fine-tune based on the binary search result and determine the final optimal input amount
```

Because the profit function is not always unimodal due to AMM curve characteristics, the search direction is dynamically switched based on the profit comparison among the three points (smaller / middle / greater).

### Dynamic flash loan selection

If the optimal input amount exceeds the APT on hand, a **flash loan** is used. The route is checked to see which LiquidSwap pair is used, and the counter coin is automatically selected in a non-conflicting order (USDC -> USDT -> BTC -> APTOGE).

---

## Relationship Between the Four Techniques

These four techniques work together across the entire MEV pipeline:

```
Technique 1: Connection ID rotation       Technique 2: Custom transaction stream
  "Keep stable connections"                 "Fetch the latest data as fast as possible"
        │                                   │
        │  High-frequency error requests ←──┘
        │  (side effect of speculative fetches)
        ▼                                   ▼
  Avoid bans with ID rotation         Fetch the latest transaction
        │                                   │
        └──── Maintain connection ──────────┘
                                            │
                                            ▼
                              Technique 3: Route filtering
                              "Find the optimal route within 15 ms"
                                            │
                                 Analyze change patterns -> narrow search -> parallel search -> optimize input amount
                                            │
                                            ▼
                              Technique 4: Move script + flash loan
                              "Execute the optimal route atomically and with capital efficiency"
                                            │
                                 Select counter coin -> execute swaps -> repay the loan
```

The **swap resource changes** in the transaction fetched fastest by Technique 2 become the input to the route filter in Technique 3. Technique 3 immediately analyzes the change pattern, narrows the search space, and finds and optimizes an arbitrage route within 15 ms. Technique 4 executes that optimal route atomically, even when the input amount exceeds the available capital, by using a flash loan. Without Technique 1's ID rotation, the speculative fetches in Technique 2 could not be maintained. Without Technique 2's fast fetching, the route discovered by Technique 3 would be overtaken by other bots. Even if Technique 3 finds an optimal input amount that exceeds the available APT, Technique 4's flash loan makes execution possible. **Only when these four techniques work together does MEV competitiveness emerge.**

---

## Technique 4: Atomic Execution and Flash Loans with a Move Script

### Design philosophy of the Move contract

oinori's on-chain execution is implemented as a Move **script**. Move has two kinds of code units, modules and scripts, and oinori chose a script for the following reasons:

- **Scripts do not need to be deployed on-chain**: modules are published to an account and anyone can inspect their interface. Scripts are sent as bytecode per transaction, so **the logic does not remain on-chain**
- **Atomic execution guarantee**: all operations inside a script execute atomically as a single transaction. If anything fails partway through, all changes are rolled back

### Four contract variants

oinori has four contract variants:

```
contracts/
  ├── oinori_swap_v1/           ← with flash loan (fixed pool_type = 0)
  ├── oinori_swap_v1_no_loan/   ← without flash loan (fixed pool_type = 0)
  ├── oinori_swap_v2/           ← with flash loan (variable pool_type)
  └── oinori_swap_v2_no_loan/   ← without flash loan (variable pool_type)
```

| Version | pool_type | Flash loan | Use case |
|---|---|---|---|
| v1 | fixed (`POOL_TYPE = 0`, AMM only) | yes | For AMM pools other than Thala |
| v1_no_loan | fixed (`POOL_TYPE = 0`) | no | Arbitrage within the APT you already have |
| v2 | variable (specified as an argument) | yes | Supports DEXs with different pool_type values, such as Thala |
| v2_no_loan | variable (specified as an argument) | no | Within existing APT + Thala support |

In v1, `pool_type` is fixed by the contract constant `POOL_TYPE = 0`, so all steps are limited to AMM pools only. In v2, `pool_type` is accepted as an argument for each step, so it can handle different pool types such as Thala's stable pool.

### Script type parameters

```move
fun main<
    Y,   // output coin of Step 1 (= input coin of Step 2)
    Z,   // output coin of Step 2 (= input coin of Step 3)
    C1,  // AMM curve type for Step 1
    C2,  // AMM curve type for Step 2
    C3,  // AMM curve type for Step 3
    BY,  // counter coin type for the flash loan
    BC   // curve type for the flash-loan pool
>(...)
```

The input coin (X) and the final output coin (OutCoin) are both hard-coded as `0x1::aptos_coin::AptosCoin`. This is because atomic arbitrage only handles circular routes that start and end with APT.

### How flash loans work

#### What a flash loan is in DeFi

A flash loan is an **unsecured loan that is borrowed and repaid within a single transaction**. In exchange for not requiring collateral, if it cannot be repaid within the same transaction, the entire transaction fails and rolls back.

#### Liquidswap's flash loan implementation

oinori uses the flash loan feature of Liquidswap (Pontem DEX). Its core is the **`Flashloan` resource**:

```move
/// A resource that cannot be copied, stored, or dropped
struct Flashloan<phantom X, phantom Y, phantom Curve> {
    x_loan: u64,
    y_loan: u64
}
```

Because of Move's **ability constraints**, this resource has none of `copy`, `store`, or `drop`. In other words:

- It cannot be copied and used twice
- It cannot be stored in global storage and reused in another transaction
- **It cannot simply be discarded** -> the only way to handle it is to consume it in `pay_flashloan`

```move
// Borrow: returns a Flashloan resource
public fun flashloan<X, Y, Curve>(x_loan: u64, y_loan: u64)
    : (Coin<X>, Coin<Y>, Flashloan<X, Y, Curve>)
{
    // ... withdraw coins from the pool
    pool.locked = true;  // prevent reentrancy
    (x_loaned, y_loaned, Flashloan<X, Y, Curve> { x_loan, y_loan })
}

// Repay: consume (destructure) the Flashloan resource
public fun pay_flashloan<X, Y, Curve>(
    x_in: Coin<X>, y_in: Coin<Y>, loan: Flashloan<X, Y, Curve>
) {
    let Flashloan { x_loan, y_loan } = loan;  // ← destructure and consume the resource
    // ... verify that LP value has not decreased
    pool.locked = false;
}
```

**The `Flashloan` resource must always be passed to `pay_flashloan` and consumed within the transaction.** This is the safety guarantee provided by Move's type system for flash loans, and it is fundamentally different from Solidity (EVM)'s `require`-based checks.

#### Flash loan flow inside the oinori script

The execution flow for the flash-loan version of the script (v1/v2) is as follows:

```
1. flashloan<BY, AptosCoin, BC>(0, coin_amount_in)
   │  -> borrow APT and receive the Flashloan resource
   │  -> the pool is locked (prevent reentrancy)
   │
2. coin::deposit(signer, coin_borrowed)
   │  -> deposit the borrowed APT into the signer account
   │
3. aggregator::swap_direct<AptosCoin, Y, Z, AptosCoin, C1, C2, C3>(...)
   │  -> execute up to 3 DEX swap steps
   │  -> circular route APT -> Y -> Z -> APT
   │
4. assert!(output >= min_out_amount)
   │  -> verify the minimum output amount
   │
5. loan_fee = borrowed_amount * 31 / 10000   (fee 0.31%)
   repay_amount = borrowed_amount + loan_fee
   │  -> calculate the flash loan fee
   │
6. deposit the intermediate coins (remaining Y and Z) into the account
   │
7. coin::withdraw<AptosCoin>(signer, repay_amount)
   │  -> withdraw the repayment amount from the account
   │
8. pay_flashloan(coin_zero, repay_coin, loan)
      -> consume the Flashloan resource and complete repayment
      -> unlock the pool
```

`coin_borrowd_zero` (the variable name is a typo in the original, but preserved here) is the counter coin on the X side at flash-loan invocation time, and since it is obtained with a borrow amount of 0, it is an empty coin. At repayment time, this empty coin and the actual repayment APT coin are passed separately to `pay_flashloan`.

### Dynamic selection of the counter coin

Because the flash loan borrows from a Liquidswap liquidity pool, the `BY` in `flashloan<BY, AptosCoin, BC>` must be the counter coin type used in the pool. In oinori, the code checks which Liquidswap pairs are used inside the arbitrage route and dynamically selects the counter coin so that **the flash-loan pool and the swap pools do not conflict**:

```rust
// oinori/oinori-mev/src/engine/find_route.rs
fn loan_info(best_route, swap_router_pool, not_use_loan_max_amount)
    -> (u64, LoanCounterCoin)
{
    // Check which Liquidswap pairs are used in the route
    let (has_usdc_pair, has_usdt_pair, has_btc_pair, has_aptoge_pair) = ...;

    // Choose a non-conflicting pair in priority order
    if !has_usdc_pair {
        (32000_0000_0000, LoanCounterCoin::Usdc)    // borrow up to 32,000 APT from the USDC/APT pool
    } else if !has_usdt_pair {
        (2000_0000_0000, LoanCounterCoin::Usdt)     // borrow up to 2,000 APT from the USDT/APT pool
    } else if !has_btc_pair {
        (1936_0000_0000, LoanCounterCoin::Btc)      // borrow up to 1,936 APT from the BTC/APT pool
    } else if !has_aptoge_pair {
        (1880_0000_0000, LoanCounterCoin::Aptoge)   // borrow up to 1,880 APT from the APTOGE/APT pool
    } else {
        (not_use_loan_max_amount, LoanCounterCoin::Usdc)  // everything conflicts -> use only own capital
    }
}
```

The USDC/APT pool has the highest liquidity, so it is chosen first and allows borrowing up to 32,000 APT. If the route uses the Liquidswap USDC/APT pair, the pool locking mechanism causes the flash loan and the swap to conflict, so it falls back to the next candidate (USDT).

### DEX aggregator interface

Swap execution uses the **Hippo Aggregator** interface. This aggregator handles routing across multiple DEXs in a unified way:

```move
// DEX identifiers
const DEX_PONTEM: u8 = 3;   // Liquidswap
const DEX_CETUS: u8 = 10;   // Cetus
const DEX_PANCAKE: u8 = 11; // PancakeSwap
const DEX_OBRIC: u8 = 12;   // Obric
// ... many others

// 1- to 3-step routing
public fun swap_direct<X, Y, Z, OutCoin, E1, E2, E3>(
    num_steps: u8,        // 1, 2, or 3
    first_dex_type: u8,   // DEX for Step 1
    first_pool_type: u64, // pool type for Step 1
    first_is_x_to_y: bool,
    // ... arguments for Step 2, Step 3
    x_in: Coin<X>
): (Option<Coin<X>>, Option<Coin<Y>>, Option<Coin<Z>>, Coin<OutCoin>)
```

`swap_direct` recursively calls `get_intermediate_output` internally and performs swaps according to the DEX of each step. Intermediate coins are returned as `Option<Coin<T>>`, and coins for unused steps are safely destroyed with `option::destroy_none`.

### How it differs from the no_loan version

The no-flash-loan versions (`oinori_swap_v1_no_loan`, `oinori_swap_v2_no_loan`) contain no borrow/repay logic at all and execute swaps using the user's own APT:

```move
// no_loan version: no flash loan
script {
    use hippo_aggregator::aggregator;

    fun main<Y, Z, C1, C2, C3, BY, BC>(
        signer: &signer, coin_amount_in: u64, min_out_amount: u64, ...
    ) {
        // Execute the swap from the APT you already have
        aggregator::swap<AptosCoin, Y, Z, AptosCoin, C1, C2, C3>(
            signer, num_steps, ...
            coin_amount_in, min_out_amount
        );
    }
}
```

On the Rust engine side, when the input amount exceeds the APT on hand, the flash-loan version is used; otherwise the no_loan version is used:

```rust
// oinori/oinori-mev/src/engine/find_route.rs
let (use_loan, loan_fee) = if input_amount > not_use_loan_max_amount {
    (true, calc_flashloan_fee(input_amount))  // 0.31% fee
} else {
    (false, 0)  // no fee
};
```

The flash loan fee (0.31%) is also precomputed on the Rust side and incorporated into the profit calculation:

```rust
// oinori/oinori-mev/src/swap/specifics/liquid/flashloan.rs
pub fn calc_flashloan_fee(amount: u64) -> u64 {
    amount * 31 / 10_000
}
```

### Mempool optimization right before execution

In atomic arbitrage, the **latency of the submission path after a route is found** directly affects the win rate. Here as well, oinori relaxes some of Aptos's standard safety-oriented processing and **immediately broadcasts to a selected validator peer right after the transaction is inserted into its own internal mempool**.

```
oinori engine
  │
  └── MempoolClientRequest::SubmitTransaction(...)
        │
        ▼
   shared mempool
     1. insert into local mempool
     2. select a low-latency peer
     3. immediate broadcast
```

The normal Aptos mempool performs VM validation and duplicate-update checks for client-submitted transactions, and then puts them on the regular broadcast schedule. In oinori, a large part of the **high-cost validation before mempool insertion is skipped**.

- **VM validation omitted**: in `shared_mempool::validate_and_add_transactions()`, instead of the normal VM validation, only the hash and size are computed, and a pseudo-success result with `status=None` is returned so the transaction is inserted into the mempool
- **Duplicate transaction update check omitted**: the duplicate check in `transaction_store.insert()` that examined whether an existing transaction for the same account and sequence number had the same payload / max_gas / expiration was removed
- **Immediate broadcast**: immediately after `SubmitTransaction` completes, `execute_broadcast()` is called for the low-latency peer selected for oinori, sending it directly instead of waiting for the normal periodic broadcast

However, **not all checks are removed**. At least the following remain:

- **Reject old sequence numbers**: if `txn.sequence_number() < db_sequence_number`, the transaction is rejected
- **Per-account capacity limit**: transactions are rejected if they exceed `capacity_per_user`

In short, assuming that the final validation happens on the validator side, oinori **cuts the pre-validation cost in its own mempool and prioritizes starting propagation as quickly as possible**. This execution optimization applies to every arbitrage transaction sent from `fire_found_route_contract()`, regardless of whether a flash loan is used.

### The role played by Move's type safety

In oinori's contract design, Move's type system automatically guarantees the following safety properties:

1. **Flash loan repayment guarantee**: because the `Flashloan` resource cannot be dropped, the code fails to compile unless it is consumed in `pay_flashloan`
2. **Coin preservation**: `Coin<T>` cannot be dropped either, so every coin must be explicitly deposited or consumed. Coin "loss" is prevented at compile time
3. **Reentrancy prevention**: the pool lock flag prevents additional operations on the same pool during the flash loan
4. **Coin-pair correctness via type parameters**: thanks to generics, the type information built by the Rust engine is used as-is in on-chain execution, and type mismatches are rejected immediately at VM execution time

---

## Summary

| Technique | Problem | Solution | Scope of change |
|---|---|---|---|
| Connection ID rotation | Bans from validators | Inject a custom private key into the Noise handshake, periodically change IDs and reconnect. Also reduce physical latency by limiting connections to Europe/Korean validators | Full network stack (9 files) |
| Custom transaction stream | Constraints on fetching the latest transaction due to `DataSummary` | Bypass StreamEngine and use `AptosDataClient` directly for speculative fetching | Custom modules inside oinori-mev |
| Route filtering + optimization | Exhaustive search does not fit within 15 ms | Search pruning with change-pattern analysis + parallel search + two-stage input amount optimization | `route_filter/`, `find_route.rs`, `optimization.rs` |
| Move script + flash loan | Executing with input amounts beyond available capital | Dynamic counter-coin selection for Liquidswap flash loans + four script variants + skipped pre-validation in mempool and immediate broadcast | `contracts/oinori_swap_v{1,2}{,_no_loan}/`, `mempool/`, `state-sync/` |

With these techniques, oinori consistently achieves everything from **low-latency transaction acquisition** to **fast route search and optimization**, and finally **capital-efficient execution using flash loans**, securing a competitive advantage in arbitrage opportunity detection and execution speed.
