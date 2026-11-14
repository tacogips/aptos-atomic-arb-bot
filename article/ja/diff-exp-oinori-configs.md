# oinori-configs ディレクトリの変更（新規追加）

Aptosメインネットのfullnodeとして動作するための設定ファイル群。oinori-mevノードが接続するメインネットの初期設定。

---

## `oinori-configs/mainnet-config/fullnode.yaml`（新規追加）
- **目的**: oinoriノード用のfullnode設定
- **主要設定**:
  - **base**: データディレクトリ（`/opt/aptos/data`）、ロール（`full_node`）、waypointファイルパス
  - **execution**: genesis.blobの場所
  - **api**: REST API有効化（`0.0.0.0:8080`で待ち受け）
  - **full_node_networks**: publicネットワークに接続、`onchain`ディスカバリー、ポート6182
  - **state_sync**: `DownloadLatestStates`ブートストラッピング、`ApplyTransactionOutputs`継続同期

```yaml
+base:
+  data_dir: "/opt/aptos/data"
+  role: "full_node"
+  waypoint:
+    from_file: "/opt/aptos/genesis/waypoint.txt"
+
+execution:
+  genesis_file_location: "/opt/aptos/genesis/genesis.blob"
+
+api:
+  enabled: true
+  address: "0.0.0.0:8080"
+
+full_node_networks:
+  - network_id: "public"
+    discovery_method: "onchain"
+    listen_address: "/ip4/0.0.0.0/tcp/6182"
+
+inspection_service:
+  address: "0.0.0.0"
+  port: 9023
+  expose_configuration: false
+  expose_system_information: true
+
+state_sync:
+  state_sync_driver:
+    bootstrapping_mode: DownloadLatestStates
+    continuous_syncing_mode: ApplyTransactionOutputs
+    num_versions_to_skip_snapshot_sync: 1000000000
+    progress_check_interval_ms: 5
+    max_pending_data_chunks: 100
+
+  data_streaming_service:
+    global_summary_refresh_interval_ms: 5
+    max_concurrent_requests: 12
+    max_concurrent_state_requests: 12
+    max_data_stream_channel_sizes: 100
+
+  aptos_data_client:
+    max_state_chunk_size: 8000
+    max_transaction_chunk_size: 6000
+    max_transaction_output_chunk_size: 3000
```

※ ファイル後半にはコメントアウトされた実験的チューニング値が多数あり（mempool設定、storage_service、aptos_data_client等）、実運用時の参考値として残されている。

---

## `oinori-configs/mainnet-config/fullnode-for-official-debug.yaml`（新規追加）
- **目的**: 公式ノードとの比較デバッグ用の最小設定
- **内容**: base, execution, full_node_networks, apiのみの設定。詳細パラメータは全てコメントアウト

```yaml
+base:
+  data_dir: "/opt/aptos/data"
+  role: "full_node"
+  waypoint:
+    from_file: "/opt/aptos/genesis/waypoint.txt"
+
+execution:
+  genesis_file_location: "/opt/aptos/genesis/genesis.blob"
+
+full_node_networks:
+  - network_id: "public"
+    discovery_method: "onchain"
+    listen_address: "/ip4/0.0.0.0/tcp/6182"
+
+api:
+  enabled: true
+  address: "0.0.0.0:8080"
```

---

## `oinori-configs/mainnet-config/genesis.blob`（新規追加、バイナリ）
- **目的**: Aptosメインネットの初期状態（genesis）ファイル
- **内容**: メインネットのジェネシスブロック状態を含むバイナリファイル（約996KB）
- **背景**: fullnodeの初回起動時にチェーンの初期状態を知る必要がある。Aptosの公式から配布されるものと同一

---

## `oinori-configs/mainnet-config/waypoint.txt`（新規追加）
- **目的**: メインネットの初期waypoint（チェックポイント）
- **背景**: ジェネシスブロック（高さ0）のハッシュ。ノード起動時にチェーンの正当性を検証するために使用

```
+0:6072b68a942aace147e0655c5704beaa255c84a7829baa4e72a500f1516584c4
```
