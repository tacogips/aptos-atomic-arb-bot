# oinori-mev ユーティリティモジュール群

アカウント管理、アドレス変換、残高差分、メトリクス、バリデータ情報などの補助モジュール。

---

## `oinori/oinori-mev/src/account/mod.rs`
- **目的**: RPCエンドポイントからのアカウント情報取得
- `get_account_via_rpc()`: RESTクライアントでアカウントデータを取得

---

## `oinori/oinori-mev/src/account_address/mod.rs`
- **目的**: hex文字列とAptos型間のアドレス変換

### `adjust_hex_address()`
- **目的**: hexアドレスを64文字（32バイト）にパディング
- **ロジック**: `0x`プレフィックスを除去し、先頭ゼロで64文字にパディング

### `hex_to_account_address()` / `hex_to_hash_value()`
- **目的**: hex文字列からAccountAddress/HashValueへの変換

---

## `oinori/oinori-mev/src/account_address/private_key.rs`
- **目的**: hexエンコードされた秘密鍵のEd25519PrivateKeyへの変換
- `hex_to_private_key()`: `0x`プレフィックス除去 → hexデコード → Ed25519PrivateKey構築

---

## `oinori/oinori-mev/src/difference/coin_diff.rs`
- **目的**: ブロックチェーンバージョン間のコイン残高差分追跡

### `CoinAmountDiff::from_versions()`
- **目的**: 複数バージョン間の残高差分を計算
- **ロジック**:
  1. 対象バージョンリスト（v1, v2, v3, ...）を受け取る
  2. v1でコイン残高をロード
  3. 各後続バージョンv(i)に対して:
     - v(i)でコイン残高をロード
     - v(i-1)と比較して差分を作成
     - v(i)を次の「before」として設定
  4. 全差分を収集して返す

---

## `oinori/oinori-mev/src/metrics/mod.rs`
- **目的**: Prometheusメトリクスエンドポイントからの同期進捗抽出

### `get_metrics()`
- **目的**: メトリクスから同期バージョンを取得
- **ロジック**:
  1. `{metrics_host}/metrics`からメトリクスをフェッチ
  2. 改行で分割
  3. 正規表現 `aptos_state_sync_version.type="synced". ([0-9]+)` でバージョンを抽出

---

## `oinori/oinori-mev/src/validator/mod.rs`
- **目的**: バリデータネットワーク情報の取得とジオロケーション

### `fetch_validators()`
- **目的**: RPCからバリデータセットを取得
- **ロジック**:
  1. `0x1::stake::ValidatorSet`リソースをRPCで取得
  2. 各バリデータ情報から:
     - アカウントアドレス（peer_id）を抽出
     - バリデータネットワークアドレスをパース
     - コンセンサス投票権を取得
  3. `OinoriValidator`リストとして返す

### `insert_validator_countries()`
- **目的**: バリデータにジオロケーション情報を付加
- **ロジック**:
  1. 各バリデータのIP/DNSアドレスを取得
     - `find_ip_addr()`を優先、なければ`find_dns()`にフォールバック
  2. `http://ip-api.com/json/{address}`にクエリ
  3. 国名・タイムゾーン・ISPを設定
  4. リクエスト間に1秒のスリープ（レートリミット）
