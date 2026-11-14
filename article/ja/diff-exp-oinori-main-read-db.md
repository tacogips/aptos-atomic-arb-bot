# oinori-mev read_db モジュール

ローカルAptosDBからの状態読み取りインターフェース。リソース・トランザクション・レジャー情報の取得を提供する。

---

## `oinori/oinori-mev/src/read_db/state.rs`
- **目的**: アカウントリソースのDB読み取り

### `get_resources_of_account()`
- **目的**: 指定バージョンでのアカウントの全リソースを取得
- **ロジック**:
  1. hexアドレスを`AccountAddress`に変換
  2. 対象バージョンを決定（明示指定またはDB最新）
  3. アドレスの`StateKeyPrefix`を作成
  4. DBのプレフィックスイテレータで状態値を走査:
     - `Resource`と`ResourceGroup`パスのみフィルタ
     - `Code`パスはスキップ
  5. `MoveConverter`で生の状態値を`MoveResource`に変換
  6. バージョンとともに返す

---

## `oinori/oinori-mev/src/read_db/tx.rs`
- **目的**: トランザクションのDB読み取り

### `get_tx_by_hash()`
- **目的**: ハッシュによるトランザクション検索
- **用途**: `start_check_transaction()`でTX確認監視に使用

### `get_tx_outputs()`
- **目的**: バッチでのトランザクション出力取得
- **制約**: `MAX_TX_OUTPUT_LIMIT = 10000`

---

## `oinori/oinori-mev/src/read_db/ledger.rs`
- **目的**: 最新レジャー情報の取得
- **ロジック**: `DbReader::get_latest_ledger_info()`でV0バリアントを抽出

---

## `oinori/oinori-mev/src/ledger/rpc.rs`
- **目的**: RPCエンドポイントからのレジャー情報取得
- **ロジック**: RESTクライアントで`get_ledger_information()`を呼び出し、`State`（バージョン・チェーン情報）を返す
