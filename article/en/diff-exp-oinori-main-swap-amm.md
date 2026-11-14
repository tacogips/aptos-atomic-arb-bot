# oinori-mev swap/amm module

Implements the AMM (automated market maker) formulas for each DEX. Supports five types: Uniswap V2, Curve Stable, Liquid Stable, Obric (piecewise AMM), and Thala (Weighted/Stable).

---

## `oinori/oinori-mev/src/swap/amm/mod.rs`
- **Purpose**: AMM abstraction layer. Defines a unified interface and routes to the appropriate implementation based on curve type

### `ReservedCoinPair::calc_amount_out()`
- **Purpose**: Main entry point for calculating output amount from input amount
- **Logic**: Dispatches to the following implementations based on `AmmCurveType`:
  - `UniswapV2` -> `get_uniswap_v2_coin_out_with_fee()`
  - `LiquidStable` -> `get_liquid_stable_coin_out_with_fee()`
  - `ObricAmm` -> `get_obric_swap_coin_out_with_fee()`
  - `ThalaAmm` -> `get_thala_swap_coin_out_with_fee()`

---

## `oinori/oinori-mev/src/swap/amm/uniswap_v2.rs`
- **Purpose**: Implementation of the Uniswap V2 constant product formula

### `get_uniswap_v2_coin_out_with_fee()`
- **Purpose**: Calculate the swap output amount including fees
- **Formula**:
  ```
  fee_amount = coin_in × fee_rate / fee_scale
  coin_in_after_fee = coin_in - fee_amount
  dy = reserve_out × coin_in_after_fee / (reserve_in + coin_in_after_fee)
  ```
  Invariant: `x × y = k` (constant product)

---

## `oinori/oinori-mev/src/swap/amm/liquid_stable.rs`
- **Purpose**: AMM implementation for stablecoins in Liquid Swap

### `get_liquid_stable_coin_out_with_fee()`
- **Purpose**: Calculate stablecoin swap output with a non-constant-product formula
- **Formula**:
  ```
  Invariant: k = x³y + xy³
  where x and y are scale-normalized reserves
  ```
- **Logic**:
  1. Apply fee: `amount_in_after_fee = amount_in × (fee_scale - fee_rate) / fee_scale`
  2. Normalize: `amount_normalized = amount × EIGHT_FIG_NUM / scale`
  3. Solve for the new reserve from the invariant

### `calc_liquid_stable_reserve_after()`
- **Purpose**: Calculate the new reserve using the Newton-Raphson method
- **Logic**:
  ```
  f(y) = x³y + xy³ - k
  f'(y) = x³ + 3xy²
  Iteration: y_new = y - f(y)/f'(y)
  ```
  Converges with at most 256 iterations

---

## `oinori/oinori-mev/src/swap/amm/obric_amm/mod.rs`
- **Purpose**: Implementation of Obric AMM (piecewise AMM). Uses different curves across three regions

### `get_obric_swap_coin_out_with_fee()`
- **Purpose**: Calculate swap output with a three-region piecewise function
- **Parameters**:
  - `k, k2`: curve constants
  - `m, n`: region boundary parameters
  - `xa, xb`: critical points that divide the curve into three regions
  - `x_deci_mult, y_deci_mult`: decimal multipliers for precision

- **Three-region logic**:
  ```
  if current_x < xa:      // upper-left region
      k2/(x - n) = (y - m)²
  elif xa ≤ x < xb:       // middle region
      k/(x + m) - m = (y + m)²
  else (x ≥ xb):          // lower-right region
      k2/(x - n) = (y - m)²
  ```

- **Normalization strategy**:
  1. Compute the significant-digit level (10⁹)
  2. Normalize all values to that level
  3. Pre-normalize to prevent overflow
  4. Denormalize the final result

---

## `oinori/oinori-mev/src/swap/amm/obric_amm/custom/mod.rs`
- **Purpose**: High-precision numerical handling for Obric AMM. Tracks normalized state with wrapper types

### Type system
- `UnPrecisedReserveIn/Out`: raw values without precision scaling
- `PrecisedReserveIn/Out`: values scaled by `PRECISION_FACTOR` (10⁶)
- `Normalizer`: manages scale conversion

### `get_swap_in_to_out()`
- **Purpose**: Main calculation using region detection
- **Logic**: Based on the current x position, dispatches to `calc_swap_output_left_or_middle()` or `calc_swap_output_bottom_right()`

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/mod.rs`
- **Purpose**: Thala AMM (supports both Weighted and Stable)

### `get_thala_swap_coin_out_with_fee()`
- **Purpose**: Route according to pool type
- **Logic**:
  - `stable_info` present -> `stable_swap_exact_in()` (uses amplification factor)
  - `pool_weight` present -> `weight_swap_exact_in()` (weighted swap)

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/thala_stable_swap.rs`
- **Purpose**: Thala stable swap (Balancer V2-style invariant)

### `compute_invariant()`
- **Purpose**: Calculate the invariant D using the Newton-Raphson method
- **Formula**:
  ```
  A × n^n × S + D = A × D × n^n + D^(n+1) / (n^n × P)
  For 2 tokens:
  A × 4 × (x + y) + D = A × D × 4 + D³ / (4xy)
  ```

### `calc_out_given_in()`
- **Purpose**: Calculate the swap output by solving for the new y that satisfies the invariant

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/thala_weighted_swap.rs`
- **Purpose**: Thala weighted swap (Balancer V2-style)

### `calc_out_given_in_weights_u64()`
- **Purpose**: Weighted output calculation
- **Formula**:
  ```
  output = (1 - ((input / (input + reserve_in)) ^ (w_in / w_out))) × reserve_out
  ```
  Uses 64-bit fixed-point arithmetic and exponentiation

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/fixed_point64/mod.rs`
- **Purpose**: 64-bit fixed-point arithmetic library for precise decimal calculations
- **Representation**: The upper 64 bits of `u128` are the fractional part

### Main functions
- `mul_fp()`: Overflow-safe multiplication using 32-bit splitting
- `div_fp()`: Safe division
- `fraction()`: Construct from numerator/denominator

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/fixed_point64/log_exp_math.rs`
- **Purpose**: Logarithmic and exponential operations for fixed-point numbers

### `log2()`
- **Purpose**: Binary logarithm
- **Algorithm**: Binary search based on `BinaryLogarithm.pdf`
  1. Normalize the input to [1, 2)
  2. Determine the fractional bits by iterating `z = (z >> 1) × (z >> 1) >> 62` 62 times

### `exp()`
- **Purpose**: Exponential function
- **Algorithm**: Multiply precomputed powers (`e^32`, `e^16`, ..., `e^(1/16)`) + Taylor series expansion (up to 8 terms)

### `pow()`
- **Purpose**: General exponentiation: `x^y = exp(y × ln(x))`
- **Optimization**: Directly compute `x^0`, `x^1`, `x^2`, and `x^4` (`try_simple_pow()`)

### `pow_up()` / `pow_down()`
- **Purpose**: Conservative exponentiation (controls the direction of rounding error)
- **Logic**: Multiply the result of `pow_internal()` by `(1 ± 10⁻⁹)`
