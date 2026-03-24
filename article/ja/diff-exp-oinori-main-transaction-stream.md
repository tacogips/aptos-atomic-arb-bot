# oinori-mev transaction_stream モジュール

ネットワークからトランザクション出力を継続的に取得し、MEVエンジンにストリーミングする。バックプレッシャー制御を含む。

---

## `oinori/oinori-mev/src/transaction_stream/mod.rs`
- **目的**: トランザクションストリームのメインループ

### `start_tx_stream()`
- **目的**: ネットワークからのトランザクション出力を継続取得するメインストリームループ
- **ロジック**:
  1. tokioインターバル（定期ポーリング用）とnotifyチャネル（即時フェッチ用）を作成
  2. メインループで両トリガーを待受:
     - タイマーtick: 定期フェッチ試行
     - notifyシグナル: 即時フェッチ試行
  3. 各フェッチで:
     - ブートストラップ同期完了を確認
     - リクエスト送信者を作成（最大同時リクエスト数超過時はNone）
     - 非同期タスクでフェッチ実行
     - フェッチサイズ計算（決定的 or ランダム範囲）
     - `get_transaction_outputs_with_proof`を実行
     - 結果をMPSCチャネルで送信
     - 次のフェッチをnotify

- **タイムアウト**: リクエストあたり45ms（`CLIENT_TIMEOUT_MS`）

---

## `oinori/oinori-mev/src/transaction_stream/stream_status.rs`
- **目的**: ストリーム状態管理とバックプレッシャー制御

### `TransactionOutputStreamStatus`構造体
- `latest_fetched_version`: 最新取得済みバージョン
- `flying_request_num`: 現在のインフライトリクエスト数

### バックプレッシャーアルゴリズム

#### `new_stream_request_sender()`
- **目的**: リクエスト送信者を作成（バックプレッシャー制御付き）
- **ロジック**:
  1. `flying_request_num > max_flying_request_num`をチェック
  2. 上限超過時: Noneを返す（バックプレッシャー適用）
  3. 上限内: 送信者を作成

#### `TransactionOutputStreamRequestSender::Drop`
- **目的**: リクエスト完了時の状態クリーンアップ
- **ロジック**:
  - `latest_fetched_version`を更新（より新しいバージョンの場合のみ）
  - `flying_request_num`をデクリメント

### `FetchTransactionRequest::as_param_versions()`
- **目的**: フェッチパラメータの計算
- **ロジック**:
  ```
  start_version = latest_fetched_version + 1
  end_version = start_version + additional_tx_fetch_size
  epoch_version = end_version
  ```
