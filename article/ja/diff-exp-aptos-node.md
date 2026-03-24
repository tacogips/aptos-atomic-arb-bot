# aptos-node パッケージの変更

Aptosノード本体の起動シーケンスを変更し、oinori-mevランタイムを組み込む。ノード起動時にMEVエンジンが自動的に起動するようにする。

---

## `aptos-node/.taplo.toml`（新規追加）
- **目的**: aptos-nodeディレクトリ固有のTOMLフォーマッタ設定
- **内容**: Cargo.lockを除外、インデント幅4スペース、列幅300文字（Cargo.tomlの長い依存関係行のため）

```diff
+#include = []
+exclude = ["**/Cargo.lock"]
+
+#https://taplo.tamasfe.dev/configuration/formatter-options.html
+[formatting]
+indent_string = "    "
+column_width = 300
```

---

## `aptos-node/Cargo.toml`

### 変更箇所: oinori-mev依存関係の追加
- **変更内容**: `oinori-mev = { path = "../oinori/oinori-mev" }` を追加
- **目的**: aptos-nodeバイナリにoinori-mevクレートを依存関係として追加。これによりノードバイナリ内でMEVエンジンを起動可能にする

```diff
@@ -78,6 +78,10 @@ serde_yaml = { workspace = true }
 tokio = { workspace = true }
 url = { workspace = true }

+
+## TODO add  oinori-mev
+oinori-mev = { path = "../oinori/oinori-mev" }
+
 [target.'cfg(unix)'.dependencies]
 jemallocator = { workspace = true }
```

---

## `aptos-node/src/lib.rs`

### 変更箇所1: `AptosHandle`構造体にoinoriランタイムフィールド追加
- **変更内容**: `_oinori_mev_runtime: Option<Runtime>` フィールドを追加
- **目的**: oinori-mevの非同期ランタイムのライフタイムをAptosノードのハンドルに紐づけ、ノードが動作している間MEVエンジンも動作し続けるようにする

```diff
@@ -207,6 +207,7 @@ pub struct AptosHandle {
     _state_sync_runtimes: StateSyncRuntimes,
     _telemetry_runtime: Option<Runtime>,
     _indexer_db_runtime: Option<Runtime>,
+    _oinori_mev_runtime: Option<Runtime>,
 }
```

### 変更箇所2: `aptos_data_client`のClone化
- **変更内容**: `aptos_data_client` → `aptos_data_client.clone()`（inspection_serviceへの引き渡し時）
- **目的**: `aptos_data_client`をoinori-mevにも渡す必要があるため、所有権を移動させずにCloneする

```diff
@@ -675,7 +677,7 @@ pub fn setup_environment_and_start_node(
     // Start the node inspection service
     services::start_node_inspection_service(
         &node_config,
-        aptos_data_client,
+        aptos_data_client.clone(),
         peers_and_metadata.clone(),
     );
```

### 変更箇所3: `mempool_client_sender`の取得
- **変更内容**: `bootstrap_api_and_indexer`の戻り値に`mempool_client_sender`を追加
- **目的**: MEVエンジンがmempoolに直接トランザクションを送信するためのチャネル（Sender）を取得する

```diff
@@ -687,6 +689,7 @@ pub fn setup_environment_and_start_node(
         indexer_runtime,
         indexer_grpc_runtime,
         internal_indexer_db_runtime,
+        mempool_client_sender, // oinori: used for mev tx submission
     ) = services::bootstrap_api_and_indexer(&node_config, db_rw.clone(), chain_id, indexer_db_opt)?;
```

### 変更箇所4: state sync完了後にoinori-mevランタイム起動
- **変更内容**: `state_sync_runtimes.block_until_initialized()` の後に `oinori_mev::start_oinori_mev()` を呼び出し
- **目的**: 状態同期が完了した後（チェーンの最新状態がローカルDBに反映された後）にMEVエンジンを起動する
- **引数**:
  - `chain_id`: チェーンID（mainnet識別用）
  - `aptos_data_client`: ピアとのデータ通信クライアント
  - `db_rw.reader`: ローカルDBの読み取りインターフェース
  - `mempool_client_sender`: mempoolへのトランザクション送信チャネル

```diff
@@ -713,11 +716,21 @@ pub fn setup_environment_and_start_node(
         &vtxn_pool,
     );

+    println!("======== oinori start sync...");
     // Wait until state sync has been initialized
     debug!("Waiting until state sync is initialized!");
     state_sync_runtimes.block_until_initialized();
     debug!("State sync initialization complete.");

+    println!("======== oinori finish sync...");
+    // Start oinori MEV runtime
+    let oinori_mev_runtime = oinori_mev::start_oinori_mev(
+        chain_id,
+        aptos_data_client,
+        Arc::clone(&db_rw.reader),
+        mempool_client_sender,
+    )?;
+
     // Create the consensus observer and publisher (if enabled)
```

### 変更箇所5: `AptosHandle`構造体の初期化にoinoriランタイム追加

```diff
@@ -759,7 +772,9 @@ pub fn setup_environment_and_start_node(
         _peer_monitoring_service_runtime: peer_monitoring_service_runtime,
         _state_sync_runtimes: state_sync_runtimes,
         _telemetry_runtime: telemetry_runtime,
+
         _indexer_db_runtime: internal_indexer_db_runtime,
+        _oinori_mev_runtime: Some(oinori_mev_runtime),
     })
 }
```

---

## `aptos-node/src/services.rs`

### 変更箇所1: Indexer起動のスキップ
- **変更内容**: `bootstrap_indexer_grpc` と `indexer::bootstrap_indexer` の呼び出しを削除し、代わりに`None`を返す
- **目的**: インデクサー関連のランタイムをスキップしてパフォーマンスを改善する。MEVノードではインデクサー機能は不要であり、リソースの節約のためスキップする

### 変更箇所2: 戻り値に`mempool_client_sender`を追加
- **変更内容**: 関数の戻り値タプルに`Sender<MempoolClientRequest>`を追加
- **目的**: mempoolクライアントの送信側チャネルを呼び出し元（lib.rs）に返し、oinori-mevがmempool経由でトランザクションを送信できるようにする

```diff
@@ -53,6 +53,7 @@ pub fn bootstrap_api_and_indexer(
     Option<Runtime>,
     Option<Runtime>,
     Option<Runtime>,
+    Sender<MempoolClientRequest>,
 )> {
     // Create the mempool client and sender
     let (mempool_client_sender, mempool_client_receiver) =
@@ -94,30 +95,16 @@ pub fn bootstrap_api_and_indexer(
         None
     };

-    // Creates the indexer grpc runtime
-    let indexer_grpc = bootstrap_indexer_grpc(
-        node_config,
-        chain_id,
-        db_rw.reader.clone(),
-        mempool_client_sender.clone(),
-        indexer_reader,
-    );
-
-    // Create the indexer runtime
-    let indexer_runtime = indexer::bootstrap_indexer(
-        node_config,
-        chain_id,
-        db_rw.reader.clone(),
-        mempool_client_sender,
-    )?;
+    // oinori: skip indexer bootstrap for performance

     Ok((
         mempool_client_receiver,
         api_runtime,
-        indexer_table_info_runtime,
-        indexer_runtime,
-        indexer_grpc,
+        None, // indexer_table_info_runtime
+        None, // indexer_runtime
+        None, // indexer_grpc
         db_indexer_runtime,
+        mempool_client_sender,
     ))
 }
```
