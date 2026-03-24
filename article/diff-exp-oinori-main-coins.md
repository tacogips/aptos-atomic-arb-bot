# oinori-mev coins モジュール

コインレジストリシステム。既知コインのメタデータ管理、単位変換、残高読み取りを提供する。

---

## `oinori/oinori-mev/src/coins/mod.rs`
- **目的**: コイン定義・レジストリ・残高操作の中央管理

### 主要な型
- `Coin`: コインのメタデータ（名前、ID、桁数、ステーブルフラグ）
- `CoinId`: コイン型文字列のラッパー（`address::module::name`形式）
- `CoinDecimal`: 精度の新型
- `CoinAmount`: コインと数量のペア
- `CoinPool`: TOMLからロードされたコインレジストリ
- `REGISTERED_COINS`: グローバルlazy初期化コインプール

### 定義済みコイン一覧
| カテゴリ | コイン |
|---|---|
| ネイティブ | APT |
| ラップド | WBTC, WUSDC, WETH |
| Thalaエコシステム | THL, MOD, ThalaAPT, AmnisApt |
| Zaption | zUSDC, zUSDT, zWETH |
| その他 | Aptoge, Pancake OFT, Tortuga StakedAptosCoin |

### `CoinAmount::from_resources()`
- **目的**: MoveリソースからAPT残高を抽出
- **ロジック**:
  1. `0x1::coin::CoinStore`タグでリソースをフィルタ
  2. ジェネリクス型パラメータからコイン型を特定
  3. `coin.value`フィールドをu64としてパース
  4. `10^decimals`で除算して整数部・小数部を算出

### `CoinAmount::load_resource()`
- **目的**: 特定バージョンでのDBからのコイン残高ロード

### `CoinPool::load()`
- **目的**: 埋め込みリソース（`resources/coin.toml`）からコインレジストリをロード

---

## `oinori/oinori-mev/src/coins/apt_coin.rs`
- **目的**: APTのOcta（最小単位）⇔人間可読単位の変換

### 定数・関数
- `APT_UNIT = 1_0000_0000`（1 APT = 10⁸ Octa）
- `apt_amount(f64)`: float APT → Octa (u64)
- `in_apt_unit(u64)`: Octa → float APT
- `in_signed_apt_unit(u64, u64)`: 符号付きOcta → float APT（正負変化量用）
