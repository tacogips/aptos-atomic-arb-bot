# oinori-mev aggregator module

Responsible for building Move script transactions for swap execution through the Hippo aggregator protocol.

---

## `oinori/oinori-mev/src/aggregator/hippo_args.rs`
- **Purpose**: convert swap routes into function arguments for the Hippo aggregator

### `DexCode` enum
DEX identifier mapping:
| Code | DEX |
|---|---|
| 8 | Pontem (Liquid) |
| 9 | Aptoswap |
| 10 | Aux |
| 11 | Animeswap |
| 12 | Cetus |
| 13 | Pancake |
| 14 | Obric |
| 15 | Thala |

### `LoanCounterCoin` enum
Selection of the flash-loan collateral coin:
- USDC / USDT / BTC / Aptoge

### `convert_hippo_args_v1()`
- **Purpose**: generate function arguments and type parameters for the V1 contract (up to 3 swap steps)
- **Logic**:
  1. Convert each swap step in the route into `DexSwapInfo`
  2. Extract the first three steps (the rest default to 0)
  3. From each step, extract:
     - DEX code, direction (`is_x_to_y`), output coin, curve type
  4. Assemble the function arguments: `input_amount`, `min_out`, `num_steps`, and parameters for each step
  5. Assemble the type tags: output coins (Y, Z), curve types (C1, C2, C3), loan counter coin, loan curve

### `convert_hippo_args_v2()`
- **Purpose**: generate arguments for the V2 contract (V1 + added `pool_type` parameter)
- **Additional logic**: each step includes `pool_type` (`u64`). Used to distinguish Thala protocol Weighted/Stable pools

---

## `oinori/oinori-mev/src/aggregator/hippo_contract.rs`
- **Purpose**: build Move script transactions for the Hippo aggregator

### Build functions
| Function | Contract | Loan |
|---|---|---|
| `build_hippo_aggregator_script_v1()` | V1 | Yes (up to 5000 gas) |
| `build_hippo_aggregator_script_v1_no_loan()` | V1 | No |
| `build_hippo_aggregator_script_v2()` | V2 | Yes |
| `build_hippo_aggregator_script_v2_no_loan()` | V2 | No |

- **Logic**: constructs `TransactionBase` from the Move package in the `contracts/oinori_swap_v{1|2}{|_no_loan}/` directory
