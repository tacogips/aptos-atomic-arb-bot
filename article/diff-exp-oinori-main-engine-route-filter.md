# oinori-mev engine/route_filter モジュール

スワップリソースの変更を分析し、ルート探索の最適なフィルタ条件を決定する。影響を受けたスワップペア数（1/2/3以上）に応じて異なるフィルタリングロジックを適用する。

---

## `oinori/oinori-mev/src/engine/route_filter/mod.rs`
- **目的**: フィルタリングのメインディスパッチャー

### `route_filter_by_affected_resource()`
- **目的**: 影響を受けたスワップ数に応じて適切なフィルタハンドラに振り分ける
- **ロジック**:
  - 1件 → `single_affect`モジュール
  - 2件 → `two_affect`モジュール（特殊パターン検出あり）
  - 3件以上 → `multiple_affect`モジュール

### `thala_resource_might_affected()`
- **目的**: Thalaコインの関与を検出
- **ロジック**: `SwapContract::Thala`またはTHL/MOD/ThalaAPT/AmnisAptの関与を検出した場合true → フィルタリングを無効化（Thalaはパターンマッチが困難なため）

---

## `oinori/oinori-mev/src/engine/route_filter/single_affect.rs`
- **目的**: 単一スワップリソース変更時のフィルタ決定

### `route_filter_by_affected_resource_single()`
- **目的**: 変更されたスワップペアの性質に基づき探索方向を決定
- **ロジック**:

  **ケース1: APT増価（APTが希少化）**
  - APTが減少（reserve_x/y decreaseかつAPT側）
  - 第1スワップフィルタ: 影響を受けたコントラクトを設定
  - ルート方向: **APT → 他コイン → ... → APT**（高く売って安く買う）
  - 入力額: [200, 6, 0.2] APT（積極的探索）

  **ケース2: APT減価（APTが過剰化）**
  - APTが増加（reserve増加かつAPT側）
  - 第2/第3スワップフィルタ: 影響を受けたコントラクトを設定
  - ルート方向: **... → 他コイン → APT**（安く買って高く売る）
  - 入力額: [200, 6, 0.2] APT

  **ケース3: 非APTペア**
  - 第2スワップフィルタを設定
  - 第2ステップの入出力コインフィルタを設定
  - 入力額: [6, 0.2] APT（通常探索）

---

## `oinori/oinori-mev/src/engine/route_filter/two_affect.rs`
- **目的**: 2つのスワップリソース変更時のフィルタ決定

### `route_filter_by_affected_resource_two()`
- **目的**: 2つの変更パターンを分析し、特殊パターンのショートカットまたは一般的分析によりフィルタを決定
- **ロジック**:

  **Thalaチェック**: 関与時はフィルタなしを返す（探索を制限しない）

  **特殊パターンショートカット** (`check_specified_route_short_cut()`):

  1. **Obricダブル**: 両方がObricのOtherタイプ → 第2スワップフィルタにObricを設定

  2. **チェーンルート**: パターン `depreciate_1 == appreciate_2 && depreciate_2 == APT`
     - 例: APT→USDC、USDC→APT
     - この正確なパターンに対する専用フィルタを構築

  3. **非APTペア**: 両更新のコインがいずれも非APT
     - 第2スワップフィルタを設定し、中間ステップのコインフィルタを構築

  **一般的分析**（ショートカット不一致時）:

  3つのカテゴリで分類:
  - **APT増価カウント**: APTが減少する更新
  - **APT減価カウント**: APTが増加する更新
  - **非APTペアカウント**: APTを含まない更新

  カウントが正確に1の場合にフィルタ設定:
  - `apt_appreciation == 1` → 第1スワップフィルタ
  - `apt_depreciation == 1` → 第3スワップフィルタ
  - `non_apt_pair == 1` → 第2スワップフィルタ

---

## `oinori/oinori-mev/src/engine/route_filter/multiple_affect.rs`
- **目的**: 3件以上のスワップリソース変更時のフィルタ決定

### `route_filter_by_affected_multiple_resource()`
- **目的**: 多数の変更を分析しフィルタを決定
- **ロジック**:
  - `two_affect`と同様のカテゴリ分析（ショートカットなし）
  - Thala関与時: フィルタなし、入力額 [50, 6, 0.2] APT
  - APT増価が1件 → 第1フィルタ設定、入力額 [50, 6, 0.2] APT
  - APT減価が1件 → 第3フィルタ設定
  - 非APTペアが1件 → 第2フィルタ設定

---

## 全体フロー図

```
route_filter_by_affected_resource()
  ├─ 1件 → single_affect
  │    ├─ APT増価 → 第1スワップフィルタ (APT→X→...→APT)
  │    ├─ APT減価 → 第3スワップフィルタ (...→X→APT)
  │    └─ 非APT → 第2スワップフィルタ + コインフィルタ
  │
  ├─ 2件 → two_affect
  │    ├─ Thala関与 → フィルタなし
  │    ├─ ショートカット検出 → 専用フィルタ
  │    └─ 一般分析 → カテゴリカウントに基づくフィルタ
  │
  └─ 3件以上 → multiple_affect
       ├─ Thala関与 → フィルタなし、拡大入力額
       └─ 一般分析 → カテゴリカウントに基づくフィルタ
```
