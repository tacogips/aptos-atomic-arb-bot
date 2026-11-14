# oinori-mev swap/specifics モジュール

各DEXプロトコルのオンチェーンリソース読み取り、イベントパース、フラッシュローン処理の実装。8つのDEX（Anime, AptoSwap, Aux, Cetus, Liquid, Obric, Pancake, Thala）をサポートする。

---

## `oinori/oinori-mev/src/swap/specifics/mod.rs`
- **目的**: `SwapContract` enumの定義と共通トレイト

### `SwapContract` enum
サポートされるDEXの識別子:
- `Anime` / `AptoSwap` / `Aux` / `Cetus` / `Liquid` / `Obric` / `Pancake` / `Thala`

各DEXのデプロイアドレス定数を定義。

---

## 各DEXの共通構造

各DEX（Anime, AptoSwap, Aux, Cetus, Obric, Pancake）は以下の3ファイルで構成:

### `{dex}/mod.rs`
- **目的**: DEX固有のコントラクトアドレスとモジュール定義
- DEXのデプロイアドレス（`AccountAddress`定数）

### `{dex}/resource.rs`
- **目的**: オンチェーンリソース（プールリザーブ）のパース
- `ConvertResource`トレイト実装: `MoveResource` → DEX固有リソース型 → `ReservedCoinPair`
- **共通ロジック**:
  1. `MoveStructTag`からDEXアドレスをマッチング
  2. ジェネリクスからコインペアの型パラメータを抽出
  3. リザーブ値（X, Y）をJSON構造体から取得
  4. 手数料率とAMMカーブタイプを設定して`ReservedCoinPair`を構築

### `{dex}/event.rs`
- **目的**: スワップイベントのパース
- ブロックチェーンイベントをDEX固有のスワップイベント型に変換

---

## DEX固有の特殊ロジック

### Liquid (`swap/specifics/liquid/`)

#### `liquid/resource.rs`
- **特殊点**: カーブタイプの判定ロジック
  - ジェネリクスの第3型パラメータ（`Curve`）を検査
  - `curves::Stable` → `AmmCurveType::LiquidStable`（ステーブルコインペア）
  - `curves::Uncorrelated` → `AmmCurveType::UniswapV2`（通常ペア）
  - スケーリングファクター（`x_scale`, `y_scale`）も抽出

#### `liquid/flashloan.rs`
- **目的**: Liquidプロトコルのフラッシュローン手数料計算
- **定数**:
  - `FLASH_LOAN_FEE = 10_000` (0.1%)
  - `FLASH_LOAN_BASE = 10_000_000`
- **関数**: `calculate_flashloan_fee(amount)` → `amount × FLASH_LOAN_FEE / FLASH_LOAN_BASE`
- フラッシュローン対応コイン: USDC, USDT, BTC, APTOGE

### Obric (`swap/specifics/obric/`)

#### `obric/resource.rs`
- **特殊点**: `ObricAmmParam`の抽出
  - `k`, `k2`, `m`, `n`, `xa`, `xb`: 3領域区分AMMの全パラメータ
  - `x_deci_mult`, `y_deci_mult`: 精度乗数

### Thala (`swap/specifics/thala/`)

#### `thala/pool_type.rs`
- **目的**: Thalaプールの分類
  - `StablePool` / `WeightedPool` の判定
  - ステーブルプール: 増幅係数（`amp_factor`）の抽出
  - 重み付きプール: ウェイト情報の抽出

#### `thala/weight.rs`
- **目的**: Thala重み付きプールのウェイト解析
- **ロジック**: ジェネリクスの型パラメータ（`Weight_5`, `Weight_10`等の命名規則）からウェイト値を計算
  - `Weight_5` → 5, `Weight_10` → 10 等
  - `weight_in / weight_out`の比率を`ThalaAmmParam`に設定

#### `thala/resource.rs`
- **特殊点**: 2つのリソースタイプ（`ThalaStableResource`, `ThalaWeightedResource`）のパース
  - ステーブル: `pool_type::AmplificationFactor`からamp_factorを抽出
  - 重み付き: `weight.rs`でウェイトを解析
  - プールアドレスが通常とは異なる構造（`metadata::CreatedAt`ジェネリクス）

---

## `oinori/oinori-mev/src/swap/events/mod.rs`
- **目的**: 全DEXスワップイベントの統合パース

### `SwapEvent::parse()`
- **目的**: ブロックチェーンイベントを統一`SwapEvent` enumにパース
- **ロジック**: 各DEXのイベント型を順次試行し、最初にマッチしたものを返す

---

## `oinori/oinori-mev/src/swap/resources/mod.rs`
- **目的**: 全DEXプールリソースの統合管理

### `SwapResource::load_resource()`
- **目的**: ブロックチェーンDBから全DEXのプールリソースをロード
- **ロジック**:
  1. 各DEXコントラクトアドレスに対して`get_resources_of_account()`を呼び出し
  2. 取得したリソースを`SwapResource`に変換
  3. 全DEXの結果を統合して返す
