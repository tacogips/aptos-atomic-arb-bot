# oinori-mev swap/amm モジュール

各DEXのAMM（自動マーケットメーカー）数式を実装する。Uniswap V2、Curve Stable、Liquid Stable、Obric（区分AMM）、Thala（Weighted/Stable）の5種類をサポートする。

---

## `oinori/oinori-mev/src/swap/amm/mod.rs`
- **目的**: AMM抽象化レイヤー。統一インターフェースを定義し、カーブタイプに応じて適切な実装にルーティングする

### `ReservedCoinPair::calc_amount_out()`
- **目的**: 入力額から出力額を計算するメインエントリーポイント
- **ロジック**: `AmmCurveType`に基づき、以下の実装に振り分ける:
  - `UniswapV2` → `get_uniswap_v2_coin_out_with_fee()`
  - `LiquidStable` → `get_liquid_stable_coin_out_with_fee()`
  - `ObricAmm` → `get_obric_swap_coin_out_with_fee()`
  - `ThalaAmm` → `get_thala_swap_coin_out_with_fee()`

---

## `oinori/oinori-mev/src/swap/amm/uniswap_v2.rs`
- **目的**: Uniswap V2のConstant Product数式実装

### `get_uniswap_v2_coin_out_with_fee()`
- **目的**: 手数料込みのスワップ出力額を計算
- **数式**:
  ```
  fee_amount = coin_in × fee_rate / fee_scale
  coin_in_after_fee = coin_in - fee_amount
  dy = reserve_out × coin_in_after_fee / (reserve_in + coin_in_after_fee)
  ```
  不変条件: `x × y = k`（Constant Product）

---

## `oinori/oinori-mev/src/swap/amm/liquid_stable.rs`
- **目的**: Liquid Swapのステーブルコイン向けAMM実装

### `get_liquid_stable_coin_out_with_fee()`
- **目的**: 非Constant Product数式によるステーブルコインスワップ計算
- **数式**:
  ```
  不変条件: k = x³y + xy³
  ここでx, yはスケール正規化されたリザーブ
  ```
- **ロジック**:
  1. 手数料適用: `amount_in_after_fee = amount_in × (fee_scale - fee_rate) / fee_scale`
  2. 正規化: `amount_normalized = amount × EIGHT_FIG_NUM / scale`
  3. 新リザーブを不変条件から求解

### `calc_liquid_stable_reserve_after()`
- **目的**: ニュートン・ラフソン法により新リザーブを計算
- **ロジック**:
  ```
  f(y) = x³y + xy³ - k
  f'(y) = x³ + 3xy²
  反復: y_new = y - f(y)/f'(y)
  ```
  最大256回の反復で収束させる

---

## `oinori/oinori-mev/src/swap/amm/obric_amm/mod.rs`
- **目的**: Obric AMM（区分AMM）の実装。3つの領域で異なるカーブを使用する

### `get_obric_swap_coin_out_with_fee()`
- **目的**: 3領域の区分関数によるスワップ出力計算
- **パラメータ**:
  - `k, k2`: カーブ定数
  - `m, n`: 領域境界パラメータ
  - `xa, xb`: カーブを3領域に分割する臨界点
  - `x_deci_mult, y_deci_mult`: 精度のための10進乗数

- **3領域ロジック**:
  ```
  if current_x < xa:      // 左上領域
      k2/(x - n) = (y - m)²
  elif xa ≤ x < xb:       // 中間領域
      k/(x + m) - m = (y + m)²
  else (x ≥ xb):          // 右下領域
      k2/(x - n) = (y - m)²
  ```

- **正規化戦略**:
  1. 有効桁数レベル（10⁹）を計算
  2. 全値をそのレベルに正規化
  3. オーバーフロー防止のための前処理正規化
  4. 最終結果を非正規化

---

## `oinori/oinori-mev/src/swap/amm/obric_amm/custom/mod.rs`
- **目的**: Obric AMMの高精度数値処理。ラッパー型による正規化状態の追跡

### 型システム
- `UnPrecisedReserveIn/Out`: 精度スケーリングなしの生値
- `PrecisedReserveIn/Out`: `PRECISION_FACTOR`（10⁶）でスケーリングされた値
- `Normalizer`: スケール変換管理

### `get_swap_in_to_out()`
- **目的**: 領域検出を用いたメイン計算
- **ロジック**: 現在のxの位置に基づき、`calc_swap_output_left_or_middle()`または`calc_swap_output_bottom_right()`に振り分け

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/mod.rs`
- **目的**: Thala AMM（Weighted + Stable両対応）

### `get_thala_swap_coin_out_with_fee()`
- **目的**: プールタイプに応じたルーティング
- **ロジック**:
  - `stable_info`あり → `stable_swap_exact_in()`（増幅係数使用）
  - `pool_weight`あり → `weight_swap_exact_in()`（重み付きスワップ）

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/thala_stable_swap.rs`
- **目的**: Thalaステーブルスワップ（Balancer V2スタイルの不変条件）

### `compute_invariant()`
- **目的**: ニュートン・ラフソン法で不変量Dを計算
- **数式**:
  ```
  A × n^n × S + D = A × D × n^n + D^(n+1) / (n^n × P)
  2トークンの場合:
  A × 4 × (x + y) + D = A × D × 4 + D³ / (4xy)
  ```

### `calc_out_given_in()`
- **目的**: 不変条件を満たす新yを求解してスワップ出力を計算

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/thala_weighted_swap.rs`
- **目的**: Thala重み付きスワップ（Balancer V2スタイル）

### `calc_out_given_in_weights_u64()`
- **目的**: 重み付き出力計算
- **数式**:
  ```
  output = (1 - ((input / (input + reserve_in)) ^ (w_in / w_out))) × reserve_out
  ```
  固定小数点64ビット演算とべき乗関数を使用

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/fixed_point64/mod.rs`
- **目的**: 精密な小数計算のための64ビット固定小数点演算ライブラリ
- **表現**: u128の上位64ビットが小数部

### 主要関数
- `mul_fp()`: 32ビット分割によるオーバーフロー安全な乗算
- `div_fp()`: 安全な除算
- `fraction()`: 分子/分母からの生成

---

## `oinori/oinori-mev/src/swap/amm/thala_amm/fixed_point64/log_exp_math.rs`
- **目的**: 固定小数点数の対数・指数演算

### `log2()`
- **目的**: 2進対数
- **アルゴリズム**: BinaryLogarithm.pdfに基づくバイナリサーチ
  1. 入力を[1,2)に正規化
  2. `z = (z >> 1) × (z >> 1) >> 62`を62回反復して小数ビットを決定

### `exp()`
- **目的**: 指数関数
- **アルゴリズム**: 事前計算されたべき乗値（e^32, e^16, ..., e^(1/16)）の乗算 + テイラー級数展開（8項まで）

### `pow()`
- **目的**: 一般べき乗: `x^y = exp(y × ln(x))`
- **最適化**: `x^0`, `x^1`, `x^2`, `x^4`は直接計算（`try_simple_pow()`）

### `pow_up()` / `pow_down()`
- **目的**: 保守的なべき乗（丸め誤差方向制御）
- **ロジック**: `pow_internal()`の結果に`(1 ± 10⁻⁹)`を乗算
