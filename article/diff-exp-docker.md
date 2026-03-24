# docker 関連ファイルの変更

oinoriノードのDockerイメージビルドおよびデプロイメント設定。

---

## `docker/builder/build-node.sh`

### 変更箇所1: `CARGO_TARGET_DIR`環境変数の追加
- **変更内容**: `CARGO_TARGET_DIR="target/${FEATURES:-"default"}"` を追加
- **目的**: features付きビルドの場合にターゲットディレクトリを分離し、ビルドキャッシュの衝突を避ける

### 変更箇所2: 軽微なフォーマット修正
- **変更内容**: `else ` → `else`（末尾の空白除去）

```diff
@@ -5,6 +5,7 @@ set -e

 PROFILE=${PROFILE:-release}
 FEATURES=${FEATURES:-""}
+CARGO_TARGET_DIR="target/${FEATURES:-"default"}"

 echo "Building aptos-node"
 echo "PROFILE: $PROFILE"
@@ -23,7 +24,7 @@ for PACKAGE in "${PACKAGES[@]}"; do
     if [ -n "$FEATURES" ] && [ "$PACKAGE" = "aptos-node" ]; then
         echo "Building aptos-node with features ${FEATURES}"
         cargo build --profile=$PROFILE --features=$FEATURES -p $PACKAGE "$@"
-    else
+    else
         # Build aptos-node separately
         cargo build --locked --profile=$PROFILE -p $PACKAGE "$@"
     fi
```

---

## `docker/builder/oinori.Dockerfile`（新規追加）
- **目的**: oinoriノード専用のDockerイメージ定義
- **構成（マルチステージビルド）**:
  1. **rust-base**: 基本的なビルド依存関係（cmake, clang, git, openssl, libpq等）をインストールしたベースイメージ
  2. **builder-base**: protobufコンパイラ（protoc）のインストール、ソースコードのコピー
  3. **aptos-node-builder**: `docker/builder/build-node.sh`を使用してaptos-nodeバイナリをビルド
  4. **validator（最終イメージ）**: ビルドされたバイナリとoinori関連設定をコピーした実行用イメージ

- **oinori固有の設定**:
  - Moveコントラクト（`oinori/oinori-mev/contracts/`）を `/opt/oinori/contracts/` にコピー
  - genesis.blob、waypoint.txt、fullnode.yamlをコピー
  - `OINORI_CONTRACT_DIR`環境変数を設定

```dockerfile
# oinoriノード固有の部分（抜粋）

### Run Validator Image
FROM rust-base AS validator
RUN ln -sf /usr/bin/perf_* /usr/bin/perf
RUN addgroup --system --gid 6180 aptos && adduser --system --ingroup aptos --no-create-home --uid 6180 aptos

RUN mkdir -p /opt/aptos/etc
COPY --link --from=aptos-node-builder /aptos/dist/aptos-node /usr/local/bin/

## --- oinori ------------------------------------------------------------------
COPY --link ./oinori/oinori-mev/contracts/ /opt/oinori/contracts/

COPY --link ./oinori-configs/mainnet-config/genesis.blob /opt/aptos/genesis/genesis.blob
COPY --link ./oinori-configs/mainnet-config/waypoint.txt /opt/aptos/genesis/waypoint.txt
COPY --link ./oinori-configs/mainnet-config/fullnode.yaml /opt/aptos/fullnode.yaml

ENV OINORI_CONTRACT_DIR /opt/oinori/contracts/
```

---

## `docker/oinori-build-rust-all.sh`（新規追加）
- **目的**: oinoriノード用のRustビルドスクリプト
- **内容**: aptos-nodeパッケージのみをビルドし、`dist/`ディレクトリにコピー
- **元のスクリプトとの違い**: 元の `docker-bake-rust-all.sh` は多数のバイナリ（tools, faucet等）をビルドするが、oinori版はaptos-nodeのみに限定してビルド時間を短縮

```bash
+#!/bin/bash
+set -e
+
+PROFILE=${PROFILE:-release}
+FEATURES=${FEATURES:-""}
+
+echo "Building aptos-node"
+echo "PROFILE: $PROFILE"
+echo "FEATURES: $FEATURES"
+echo "CARGO_TARGET_DIR: $CARGO_TARGET_DIR"
+
+if [ -n "$FEATURES" ]; then
+    echo "Building aptos-node with features ${FEATURES}"
+    cargo build --profile=$PROFILE --features=$FEATURES -p aptos-node "$@"
+else
+    cargo build --locked --profile=$PROFILE \
+        -p aptos-node \
+        "$@"
+fi
+
+BINS=(
+    aptos-node
+)
+
+mkdir dist
+
+for BIN in "${BINS[@]}"; do
+    cp target/$PROFILE/$BIN dist/$BIN
+done
```
