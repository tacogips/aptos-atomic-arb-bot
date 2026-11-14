# oinori-mev coins module

Coin registry system. Provides metadata management for known coins, unit conversion, and balance reads.

---

## `oinori/oinori-mev/src/coins/mod.rs`
- **Purpose**: central management of coin definitions, registry, and balance operations

### Main types
- `Coin`: coin metadata (name, ID, decimals, stable flag)
- `CoinId`: wrapper for a coin type string (`address::module::name` format)
- `CoinDecimal`: newtype for precision
- `CoinAmount`: pair of coin and amount
- `CoinPool`: coin registry loaded from TOML
- `REGISTERED_COINS`: globally lazy-initialized coin pool

### Defined coin list
| Category | Coins |
|---|---|
| Native | APT |
| Wrapped | WBTC, WUSDC, WETH |
| Thala ecosystem | THL, MOD, ThalaAPT, AmnisApt |
| Zaption | zUSDC, zUSDT, zWETH |
| Other | Aptoge, Pancake OFT, Tortuga StakedAptosCoin |

### `CoinAmount::from_resources()`
- **Purpose**: extract APT balances from Move resources
- **Logic**:
  1. Filter resources by the `0x1::coin::CoinStore` tag
  2. Identify the coin type from the generic type parameter
  3. Parse the `coin.value` field as `u64`
  4. Divide by `10^decimals` to calculate integer and fractional parts

### `CoinAmount::load_resource()`
- **Purpose**: load a coin balance from the DB at a specific version

### `CoinPool::load()`
- **Purpose**: load the coin registry from the embedded resource (`resources/coin.toml`)

---

## `oinori/oinori-mev/src/coins/apt_coin.rs`
- **Purpose**: convert between APT Octa (smallest unit) and human-readable units

### Constants and functions
- `APT_UNIT = 1_0000_0000` (`1 APT = 10^8 Octa`)
- `apt_amount(f64)`: float APT -> Octa (`u64`)
- `in_apt_unit(u64)`: Octa -> float APT
- `in_signed_apt_unit(u64, u64)`: signed Octa -> float APT (for positive/negative deltas)
