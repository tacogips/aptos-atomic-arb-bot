# oinori-mev swap/router module

The swap routing engine. It is responsible for searching and optimizing arbitrage paths across multiple DEX pools.

---

## `oinori/oinori-mev/src/swap/router/coin_pair_pool.rs`
- **Purpose**: Manage coin pairs within a single DEX pool. Provides O(1) bidirectional lookup via nested `HashMap`s

### Main data structures
- `CoinPairPool`: Keeps all coin pairs for a single DEX in a bidirectional index
- `UpdatePairType`: Classification of pool updates
  - `XIncreaseYDecrease` / `XDecreaseYIncrease`: Typical swaps
  - `XIncreaseYIncrease` / `XDecreaseYDecrease`: Liquidity additions/removals
  - `NewPair` / `Other`: New pair additions, etc.

### `get_routes_by_input_x()` / `get_routes_by_input_y()`
- **Purpose**: Return swap routes as iterators for the specified coin as input
- **Logic**: O(1) lookup from the internal `HashMap` structure `coin_id -> HashMap<other_coin_id, pair_index>`

---

## `oinori/oinori-mev/src/swap/router/swap_route_pool.rs`
- **Purpose**: Unified management of all DEX pools and cross-DEX routing

### Main data structures
- `SwapRouterPool`: Integrates all DEX pools
- `SwapRoute`: A complete swap path (multiple steps)
- `SwapPairIndex`: Represented as `u8` (swap) + `u16` (pair) to minimize memory usage

### `SwapRouterPool::load_from_db()`
- **Purpose**: Load all DEX pools from the blockchain DB
- **Logic**: Retrieve resources for all supported DEXs with `SwapResource::load_resource()` and convert them into `CoinPairPool`

### `SwapRouterPool::update_pool_with_resources()`
- **Purpose**: Reflect user transaction swap resource changes in the pool
- **Return value**: `Vec<(SwapContract, UpdatePairType)>` - the list of affected swap pairs

### `SwapRouterPool::get_routes_from()`
- **Purpose**: Return all routes from the specified coin as an iterator
- **Logic**: Sequentially traverse `get_routes_by_input_x/y` across all DEX pools

---

## `oinori/oinori-mev/src/swap/router/traverser.rs`
- **Purpose**: Route search algorithm. Discovers all arbitrage paths via asynchronous recursive depth-first search

### `start_route_traverse()`
- **Purpose**: Main entry point for route search
- **Parameters**:
  - `input_amounts`: List of input amounts to test
  - `timeout`: Search timeout (default 15ms)
  - `max_depth`: Maximum depth (default 3 steps)
  - `swap_filters`: DEX filters for each step `(SwapContract, SwapContract, SwapContract)`
  - `coin_filters`: Coin filters for each step
  - `exclude_swap`: DEXs to exclude

- **Algorithm**:
  ```
  For each input_amount:
    For each starting DEX pool index:
      Spawn an asynchronous task:
        Call traverse() with the route for the first step
        traverse() recursively:
          1. Walk all possible next routes
          2. For each route: calculate output amount
          3. If destination is reached: send result through channel
          4. If maximum depth has not been reached: recurse deeper
  Main task: Select the most profitable route within the timeout
  ```

### `traverse()` (internal function)
- **Purpose**: Recursively search routes in depth-first order
- **Logic**:
  1. Detect cycles using `visited_routes`
  2. For each candidate route, compute output amount with `calc_amount_out()`
  3. If output > 0 and the input coin is reached, send it as a complete route
  4. If within the depth limit, continue searching the next step recursively
  5. Choose the most profitable route from all concurrent search results within the timeout

---

## `oinori/oinori-mev/src/swap/router/optimization.rs`
- **Purpose**: Optimize the input amount for a fixed route. Finds the maximum-profit input amount with ternary search

### `optimize_route_amount_with_loan_fee()`
- **Purpose**: Search for the optimal input amount while accounting for loan fees
- **Parameters**:
  - `linear_step`: Step size (e.g. 0.3 APT)
  - `max_steps`: Maximum number of steps (e.g. 500)
  - `loan_fee_fn`: Loan fee calculation function

- **Algorithm**:

  **Phase 1: Binary search (3 test points)**
  ```
  middle = initial input amount
  smaller = middle / 2
  greater = middle * 2

  Loop:
    Calculate profit at 3 points
    If the best point is:
      smaller -> narrow the search range to the left
      greater -> expand the search range to the right
      middle -> converged (move to phase 2)
  ```

  **Phase 2: Linear fine-tuning**
  ```
  Starting from the convergence point, increase/decrease by linear_step:
    Step in the direction that increases profit
    Stop when profit decreases
  ```

### `calc_amounts_output()`
- **Purpose**: Simulate the swap output for the entire route
- **Logic**: Apply each step's `calc_amount_out()` in sequence, then subtract the loan fee from the final output
