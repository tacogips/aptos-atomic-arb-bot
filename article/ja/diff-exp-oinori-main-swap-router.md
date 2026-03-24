# oinori-mev swap/router モジュール

スワップルーティングエンジン。複数DEXプールを横断するアービトラージ経路の探索・最適化を担う。

---

## `oinori/oinori-mev/src/swap/router/coin_pair_pool.rs`
- **目的**: 単一DEXプール内のコインペア管理。ネストされたHashMapによりO(1)での双方向検索を提供

### 主要データ構造
- `CoinPairPool`: 単一DEXの全コインペアを双方向インデックスで保持
- `UpdatePairType`: プール更新の分類
  - `XIncreaseYDecrease` / `XDecreaseYIncrease`: 典型的なスワップ
  - `XIncreaseYIncrease` / `XDecreaseYDecrease`: 流動性追加/除去
  - `NewPair` / `Other`: 新規ペア追加等

### `get_routes_by_input_x()` / `get_routes_by_input_y()`
- **目的**: 指定コインを入力としたスワップルートをイテレータで返す
- **ロジック**: 内部HashMap構造 `coin_id → HashMap<other_coin_id, pair_index>` からO(1)でルックアップ

---

## `oinori/oinori-mev/src/swap/router/swap_route_pool.rs`
- **目的**: 全DEXプールの統合管理とクロスDEXルーティング

### 主要データ構造
- `SwapRouterPool`: 全DEXプールを統合
- `SwapRoute`: 完全なスワップ経路（複数ステップ）
- `SwapPairIndex`: メモリ最小化のためu8（スワップ）+ u16（ペア）で表現

### `SwapRouterPool::load_from_db()`
- **目的**: ブロックチェーンDBから全DEXプールをロード
- **ロジック**: `SwapResource::load_resource()`で全対応DEXのリソースを取得し、`CoinPairPool`に変換

### `SwapRouterPool::update_pool_with_resources()`
- **目的**: ユーザートランザクションのスワップリソース変更をプールに反映
- **戻り値**: `Vec<(SwapContract, UpdatePairType)>` — 影響を受けたスワップペアの一覧

### `SwapRouterPool::get_routes_from()`
- **目的**: 指定コインからの全ルートをイテレータで返す
- **ロジック**: 全DEXプールの`get_routes_by_input_x/y`を順次走査

---

## `oinori/oinori-mev/src/swap/router/traverser.rs`
- **目的**: ルート探索アルゴリズム。非同期再帰による深さ優先探索で全アービトラージ経路を発見する

### `start_route_traverse()`
- **目的**: ルート探索のメインエントリーポイント
- **パラメータ**:
  - `input_amounts`: テストする入力額リスト
  - `timeout`: 探索タイムアウト（デフォルト15ms）
  - `max_depth`: 最大深度（デフォルト3ステップ）
  - `swap_filters`: 各ステップのDEXフィルタ `(SwapContract, SwapContract, SwapContract)`
  - `coin_filters`: 各ステップのコインフィルタ
  - `exclude_swap`: 除外DEX

- **アルゴリズム**:
  ```
  各input_amountに対して:
    各開始DEXプールインデックスに対して:
      非同期タスクを起動:
        traverse()を第1ステップのルートで呼び出し
        traverse()は再帰的に:
          1. 全可能な次ルートを走査
          2. 各ルート: 出力額を計算
          3. 宛先到達: チャネルで結果送信
          4. 最大深度未到達: さらに深く再帰
  メインタスク: タイムアウト内で最良利益のルートを選択
  ```

### `traverse()`（内部関数）
- **目的**: 深さ優先でルートを再帰的に探索
- **ロジック**:
  1. `visited_routes`によるサイクル検出
  2. 各候補ルートに対して`calc_amount_out()`で出力額計算
  3. 出力 > 0 かつ入力コインに到達した場合、完全ルートとして送信
  4. 深度制限内の場合、再帰で次ステップを探索
  5. 並行探索の全結果からタイムアウト内で最大利益のルートを選択

---

## `oinori/oinori-mev/src/swap/router/optimization.rs`
- **目的**: 固定ルートに対する入力額の最適化。三分探索で最大利益の入力額を発見する

### `optimize_route_amount_with_loan_fee()`
- **目的**: ローン手数料を考慮した最適入力額の探索
- **パラメータ**:
  - `linear_step`: ステップ幅（例: 0.3 APT）
  - `max_steps`: 最大ステップ数（例: 500）
  - `loan_fee_fn`: ローン手数料計算関数

- **アルゴリズム**:

  **フェーズ1: 二分探索（3テストポイント）**
  ```
  middle = 初期入力額
  smaller = middle / 2
  greater = middle × 2

  ループ:
    3点で利益計算
    最良点が:
      smaller → 探索範囲を左に縮小
      greater → 探索範囲を右に拡大
      middle → 収束（フェーズ2へ）
  ```

  **フェーズ2: 線形微調整**
  ```
  収束点からlinear_stepずつ増減しながら:
    利益が増加する方向にステップ
    利益が減少したら停止
  ```

### `calc_amounts_output()`
- **目的**: ルート全体のスワップ出力をシミュレーション
- **ロジック**: 各ステップの`calc_amount_out()`を順次適用し、最終出力からローン手数料を減算
