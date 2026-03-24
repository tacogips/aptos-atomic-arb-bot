# oinori-mev lib.rs / エントリーポイント

oinori-mevクレートのエントリーポイント。MEVエンジン全体の起動・初期化・オーケストレーションを行う。

---

## `oinori/oinori-mev/src/lib.rs`
- **目的**: MEVエンジンのランタイム起動とトランザクションストリームの接続

### `start_oinori_mev()`
- **目的**: マルチスレッドTokioランタイムを作成し、MEVエンジンを起動する
- **ロジック**: `tokio::runtime::Builder`で専用ランタイムを構築（LIFO slot無効化）し、`mev_transaction_handler()`をブロッキング実行する

### `mev_transaction_handler()`
- **目的**: MEVエンジンの全コンポーネントを起動・接続するメインハンドラ
- **ロジック**:
  1. `OinoriEnvs`から環境設定をロード
  2. MPSCチャネルを作成（トランザクションストリーム→MEVエンジンの通信用）
  3. 3つの非同期タスクを並行起動:
     - `setup_and_keep_sync_engine`: エンジンの初期化とブロックチェーン同期維持
     - `start_mev_engine`: トランザクションを受信してMEVロジックを実行
     - `start_tx_stream`: ネットワークからトランザクション出力を継続取得
  4. メインループでストリーム受信データを`MoveConverter`で`UserTransactionLog`に変換

---

## `oinori/oinori-mev/src/envs/mod.rs`
- **目的**: 環境変数によるMEVエンジン設定管理

### `OinoriEnvs`構造体
| 環境変数 | フィールド | デフォルト値 |
|---|---|---|
| `OINORI_ACCOUNT_SET_ID` | `oinori_sender_set_id` | `"default"` |
| `OINORI_RPC_URL` | `rpc_url` | `"https://fullnode.mainnet.aptoslabs.com"` |
| `OINORI_METRICS_URL` | `self_metrics_url` | `"http://localhost:9023"` |
| `OINORI_NODE_ID` | `node_id` | `"default_node"` |
| `OINORI_CONTRACT_DIR` | `contract_dir` | `"{CARGO_MANIFEST_DIR}/contracts"` |
| `OINORI_MEV_DISABLED` | `oinori_mev_disable` | `false` |
| `OINORI_MEV_DRY_RUN` | `oinori_mev_dry_run` | `false` |

---

## `oinori/oinori-mev/src/path.rs`
- **目的**: `include_dir!`マクロによるプロジェクトリソース（coin.toml等）のコンパイル時埋め込み

---

## `oinori/oinori-mev/src/hasher/mod.rs`
- **目的**: `FxHashMap`/`FxHashSet`の再エクスポート。暗号的安全性よりパフォーマンスを優先したハッシュ実装を内部データ構造に使用

---

## `oinori/oinori-mev/src/resource.rs`
- **目的**: Moveの型システムとRust間の変換ヘルパー。MoveリソースをJSONとして扱うためのマクロ・関数群を提供
- **主要マクロ**: `move_types_vec_to_struct!`, `get_value_from_json_obj!`, `get_value_from_struct_with_type!`
