# oinori-mev aggregator モジュール

Hippoアグリゲータープロトコルを通じたスワップ実行用のMove スクリプトトランザクション構築を担う。

---

## `oinori/oinori-mev/src/aggregator/hippo_args.rs`
- **目的**: スワップルートをHippoアグリゲーターの関数引数に変換

### `DexCode` enum
DEX識別子のマッピング:
| コード | DEX |
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
フラッシュローン担保コインの選択:
- USDC / USDT / BTC / Aptoge

### `convert_hippo_args_v1()`
- **目的**: V1コントラクト用の関数引数・型パラメータを生成（最大3ステップスワップ）
- **ロジック**:
  1. ルートの各スワップステップを`DexSwapInfo`に変換
  2. 最初の3ステップを抽出（残りはデフォルト値0）
  3. 各ステップから抽出:
     - DEXコード、方向（`is_x_to_y`）、出力コイン、カーブタイプ
  4. 関数引数を組み立て: `input_amount`, `min_out`, `num_steps`, 各ステップパラメータ
  5. 型タグを組み立て: 出力コイン（Y, Z）、カーブタイプ（C1, C2, C3）、ローンカウンターコイン、ローンカーブ

### `convert_hippo_args_v2()`
- **目的**: V2コントラクト用の引数生成（V1 + pool_typeパラメータ追加）
- **追加ロジック**: 各ステップに`pool_type`（u64）を含む。ThalaプロトコルのWeighted/Stable判別に使用

---

## `oinori/oinori-mev/src/aggregator/hippo_contract.rs`
- **目的**: Hippoアグリゲーター用Moveスクリプトトランザクションのビルド

### ビルド関数群
| 関数 | コントラクト | ローン |
|---|---|---|
| `build_hippo_aggregator_script_v1()` | V1 | あり（最大5000 gas） |
| `build_hippo_aggregator_script_v1_no_loan()` | V1 | なし |
| `build_hippo_aggregator_script_v2()` | V2 | あり |
| `build_hippo_aggregator_script_v2_no_loan()` | V2 | なし |

- **ロジック**: `contracts/oinori_swap_v{1|2}{|_no_loan}/`ディレクトリのMoveパッケージから`TransactionBase`を構築
