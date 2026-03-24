# oinori-mev swap/specifics module

Implementation of on-chain resource reading, event parsing, and flash-loan handling for each DEX protocol. Supports eight DEXs: Anime, AptoSwap, Aux, Cetus, Liquid, Obric, Pancake, and Thala.

---

## `oinori/oinori-mev/src/swap/specifics/mod.rs`
- **Purpose**: Definition of the `SwapContract` enum and common traits

### `SwapContract` enum
Identifiers for the supported DEXs:
- `Anime` / `AptoSwap` / `Aux` / `Cetus` / `Liquid` / `Obric` / `Pancake` / `Thala`

Defines constants for each DEX deployment address.

---

## Common structure for each DEX

Each DEX (Anime, AptoSwap, Aux, Cetus, Obric, Pancake) consists of the following three files:

### `{dex}/mod.rs`
- **Purpose**: DEX-specific contract address and module definitions
- DEX deployment address (`AccountAddress` constant)

### `{dex}/resource.rs`
- **Purpose**: Parse on-chain resources (pool reserves)
- Implements the `ConvertResource` trait: `MoveResource` -> DEX-specific resource type -> `ReservedCoinPair`
- **Common logic**:
  1. Match the DEX address from `MoveStructTag`
  2. Extract coin pair type parameters from generics
  3. Obtain reserve values (X, Y) from the JSON structure
  4. Build `ReservedCoinPair` by setting the fee rate and AMM curve type

### `{dex}/event.rs`
- **Purpose**: Parse swap events
- Convert blockchain events into the DEX-specific swap event type

---

## DEX-specific special logic

### Liquid (`swap/specifics/liquid/`)

#### `liquid/resource.rs`
- **Special point**: Curve type determination logic
  - Inspect the third generic type parameter (`Curve`)
  - `curves::Stable` -> `AmmCurveType::LiquidStable` (stablecoin pair)
  - `curves::Uncorrelated` -> `AmmCurveType::UniswapV2` (regular pair)
  - Scaling factors (`x_scale`, `y_scale`) are also extracted

#### `liquid/flashloan.rs`
- **Purpose**: Calculate flash-loan fees for the Liquid protocol
- **Constants**:
  - `FLASH_LOAN_FEE = 10_000` (0.1%)
  - `FLASH_LOAN_BASE = 10_000_000`
- **Function**: `calculate_flashloan_fee(amount)` -> `amount * FLASH_LOAN_FEE / FLASH_LOAN_BASE`
- Flash-loan supported coins: USDC, USDT, BTC, APTOGE

### Obric (`swap/specifics/obric/`)

#### `obric/resource.rs`
- **Special point**: Extraction of `ObricAmmParam`
  - `k`, `k2`, `m`, `n`, `xa`, `xb`: all parameters for the three-region segmented AMM
  - `x_deci_mult`, `y_deci_mult`: precision multipliers

### Thala (`swap/specifics/thala/`)

#### `thala/pool_type.rs`
- **Purpose**: Classify Thala pools
  - Determine `StablePool` / `WeightedPool`
  - Stable pools: extract amplification factor (`amp_factor`)
  - Weighted pools: extract weight information

#### `thala/weight.rs`
- **Purpose**: Parse weights for Thala weighted pools
- **Logic**: Calculate weight values from the generic type parameter naming convention (`Weight_5`, `Weight_10`, etc.)
  - `Weight_5` -> 5, `Weight_10` -> 10, etc.
  - Set the `weight_in / weight_out` ratio in `ThalaAmmParam`

#### `thala/resource.rs`
- **Special point**: Parse two resource types (`ThalaStableResource`, `ThalaWeightedResource`)
  - Stable: extract `amp_factor` from `pool_type::AmplificationFactor`
  - Weighted: parse weights in `weight.rs`
  - The pool address has a structure different from usual (`metadata::CreatedAt` generic)

---

## `oinori/oinori-mev/src/swap/events/mod.rs`
- **Purpose**: Integrated parsing of swap events for all DEXs

### `SwapEvent::parse()`
- **Purpose**: Parse blockchain events into the unified `SwapEvent` enum
- **Logic**: Try each DEX event type in sequence and return the first match

---

## `oinori/oinori-mev/src/swap/resources/mod.rs`
- **Purpose**: Integrated management of all DEX pool resources

### `SwapResource::load_resource()`
- **Purpose**: Load pool resources for all DEXs from the blockchain DB
- **Logic**:
  1. Call `get_resources_of_account()` for each DEX contract address
  2. Convert the retrieved resources into `SwapResource`
  3. Integrate and return the results from all DEXs
