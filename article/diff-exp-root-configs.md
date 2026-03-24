# ルートディレクトリ設定ファイルの変更

aptos-coreフォーク上でoinori-mevアトミックアービトラージエンジンを動作させるために追加・変更されたプロジェクトルートの設定ファイル群。

---

## 新規追加ファイル

### `.envrc`（新規追加）
- **目的**: Nix開発環境の自動読み込み
- **内容**: `use flake .` — ディレクトリに入った際にflake.nixで定義された開発シェルを自動的にアクティブにする（direnv連携）

```diff
+use flake .
```

---

### `.taplo.toml`（新規追加）
- **目的**: TOMLファイルのフォーマッタ設定
- **内容**: Cargo.lockを除外し、インデント幅4スペース、列幅80文字に設定。Cargo.toml編集時のフォーマット統一のため

```diff
+#include = []
+exclude = ["**/Cargo.lock"]
+
+#https://taplo.tamasfe.dev/configuration/formatter-options.html
+[formatting]
+indent_string = "    "
+column_width = 80
```

---

### `CLAUDE.md`（新規追加）
- **目的**: Claude Code（AIアシスタント）向けのプロジェクト指示ファイル
- **内容**:
  - `git diff main`時にCargo.lockを除外するポリシー
  - 教育資料としての公開準備が目的であることの明記
  - クリーンアップチェックリスト（認証情報の削除、ハードコードされたアドレスの削除等）
  - README等に記載すべき免責事項のリスト

```diff
+# Project Instructions
+
+## Git Diff Policy
+
+- When comparing against `main` branch, always exclude `Cargo.lock` from the diff:
+  ```
+  git diff main -- ':!Cargo.lock'
+  ```
+- Cargo.lock is large and contains many dependency changes that are not relevant to code review
+
+## Project Goal
+
+The purpose of modifications on this branch is to prepare the codebase for public release as an **educational material for atomic arbitrage**.
+
+### Objectives
+
+1. Remove unnecessary items from the diff between `feature/to_book` and `main`
+2. Clean up the code to be suitable as a general atomic arb tutorial/reference
+3. Ensure no sensitive information remains in the code
+
+### Cleanup Checklist
+
+- [ ] Remove all credential information (private keys, API keys, tokens)
+- [ ] Remove hardcoded addresses that are not publicly known
+- [ ] Remove unnecessary debug code and commented-out code
+- [ ] Remove environment-specific configurations
+- [ ] Keep only essential code for demonstrating atomic arbitrage concepts
+
+## Disclaimer Requirements
+
+When modifying README.md or any documentation, ensure the following disclaimers are always included (in both English and Japanese):
+
+1. **Fork Origin**: This is a fork of aptos-core with oinori-mev atomic arbitrage functionality
+2. **No Warranty**: Code worked at past commits but no guarantee for current state
+3. **Educational Only**: This is learning/educational code only
+4. **No Financial Responsibility**: Creator accepts no responsibility for financial losses
+5. **Not Investment Advice**: Does not constitute investment advice, does not recommend investment
```

---

### `Taskfile.yml`（新規追加）
- **目的**: go-taskによるDockerビルド・実行のタスクランナー定義
- **内容**:
  - `build`: Docker buildxでaptos-oinoriイメージをビルド
  - `run`: 標準モードでoinoriノードを起動
  - `run-dry`: ドライランモード（実際のトランザクションを送信しない）で起動
  - `stop`: コンテナの停止
  - `check`: ノードの稼働確認（API疎通チェック）
  - `remove-lock-files`: クラッシュ後のDBロックファイル削除

```diff
+# Oinori MEV - Docker Build and Run
+#
+# Prerequisites:
+# - Docker with buildx support
+# - See: https://github.com/docker/buildx
+# - go-task: https://taskfile.dev
+
+version: '3'
+
+tasks:
+  build:
+    desc: Build the Docker image locally
+    cmds:
+      - >-
+        docker buildx build --load --platform linux/amd64
+        --build-arg BUILT_VIA_BUILDKIT=true
+        -f docker/builder/oinori.Dockerfile
+        -t aptos-oinori:latest .
+
+  run:
+    desc: Run oinori node in Docker (standard mode)
+    deps: [build]
+    cmds:
+      - docker compose up oinori
+
+  run-dry:
+    desc: Run oinori node in Docker (dry-run mode - no actual transactions)
+    deps: [build]
+    cmds:
+      - docker compose up oinori-dry-run
+
+  stop:
+    desc: Stop all containers
+    cmds:
+      - docker compose down
+
+  check:
+    desc: Check if node is running
+    cmds:
+      - curl localhost:8080/v1
+
+  remove-lock-files:
+    desc: Remove database lock files (if container crashed)
+    cmds:
+      - sudo rm -f ./data/aptos_mainnet_data/db/ledger_db/LOCK
+      - sudo rm -f ./data/aptos_mainnet_data/db/state_merkle_db/LOCK
```

---

### `docker-compose.yml`（新規追加）
- **目的**: oinoriノードのDocker Compose定義
- **内容**:
  - `oinori`: 標準モードのMEVノード。ホストネットワークモード、データボリュームマウント、fullnode.yamlで起動
  - `oinori-dry-run`: ドライランモード。環境変数 `OINORI_MEV_DRY_RUN=1` を設定し、実際のトランザクション送信を行わない
  - 両コンテナともポート8080（API）、9023（Inspection）、16182（P2P）を公開

```diff
+services:
+  # Oinori MEV node - standard mode
+  oinori:
+    build:
+      context: .
+      dockerfile: docker/builder/oinori.Dockerfile
+      args:
+        BUILT_VIA_BUILDKIT: "true"
+
+    environment:
+      OINORI_NODE_ID: oinori_node
+      RUST_LOG: info
+
+    volumes:
+      - ./data/aptos_mainnet_data:/opt/aptos/data
+
+    command: ["/usr/local/bin/aptos-node", "-f", "/opt/aptos/fullnode.yaml"]
+
+    network_mode: "host"
+
+    expose:
+      - 8080
+      - 9023
+      - 16182
+
+    restart: unless-stopped
+
+  # Oinori MEV node - dry-run mode (no actual transactions submitted)
+  oinori-dry-run:
+    build:
+      context: .
+      dockerfile: docker/builder/oinori.Dockerfile
+      args:
+        BUILT_VIA_BUILDKIT: "true"
+
+    environment:
+      OINORI_NODE_ID: oinori_node_dry_run
+      OINORI_MEV_DRY_RUN: 1
+      RUST_LOG: info
+
+    volumes:
+      - ./data/aptos_mainnet_data:/opt/aptos/data
+
+    command: ["/usr/local/bin/aptos-node", "-f", "/opt/aptos/fullnode.yaml"]
+
+    network_mode: "host"
+
+    expose:
+      - 8080
+      - 9023
+      - 16182
+
+    restart: unless-stopped
```

---

### `flake.nix`（新規追加）
- **目的**: Nix Flakesによる再現可能な開発環境の定義
- **内容**:
  - Rustツールチェーン（fenix経由でrust-toolchain.tomlから読み込み）
  - ネイティブビルド依存関係（pkg-config, binutils, gcc, lld, openssl, zlib等）
  - 開発ツール（go-task, nodejs等）
  - `LIBCLANG_PATH`やLD_LIBRARY_PATHの設定
  - `docker buildx use default`の自動実行

```diff
+{
+  description = "Oinori - Educational atomic arbitrage bot implementation on Aptos blockchain.";
+
+  inputs = {
+    nixpkgs.url = "github:NixOS/nixpkgs/release-24.11";
+    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
+    flake-utils.url = "github:numtide/flake-utils";
+    fenix.url = "github:nix-community/fenix";
+  };
+
+  outputs = { self, nixpkgs, nixpkgs-unstable, flake-utils, fenix }:
+    flake-utils.lib.eachDefaultSystem (system:
+      let
+        overlays = [ fenix.overlays.default ];
+        pkgs = import nixpkgs { inherit system overlays; };
+        pkgs-unstable = import nixpkgs-unstable { inherit system; };
+        rust-components = fenix.packages.${system}.fromToolchainFile {
+          file = ./rust-toolchain.toml;
+          sha256 = "sha256-opUgs6ckUQCyDxcB9Wy51pqhd0MPGHUVbwRKKPGiwZU=";
+        };
+      in
+      {
+        devShells.default = pkgs.mkShell {
+          nativeBuildInputs = [
+            pkgs.pkg-config
+            pkgs.binutils
+            pkgs.gcc
+            pkgs.lld
+          ];
+
+          buildInputs = [
+            rust-components
+            pkgs.udev
+            pkgs.zlib
+            pkgs.git
+            pkgs.llvmPackages.libclang.lib
+            pkgs.openssl
+            pkgs.zlib
+            pkgs.elfutils
+            pkgs.postgresql.lib
+            pkgs-unstable.nodejs_22
+            pkgs.go-task
+          ];
+
+          shellHook = ''
+            docker buildx use default
+            export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
+            export LD_LIBRARY_PATH="..."  # 各ライブラリのパスを設定
+          '';
+        };
+      }
+    );
+}
```

`flake.lock`はflake.nixの依存関係を固定するロックファイル（自動生成）。

---

## 変更ファイル

### `.dockerignore`
- **変更箇所**: `!oinori` と `!oinori-configs` を追加
- **目的**: Dockerビルド時にoinori関連ディレクトリがコンテキストに含まれるようにする（.dockerignoreはデフォルトで全てを除外する設定のため、必要なディレクトリを明示的に許可）

```diff
@@ -41,3 +41,6 @@
 !types/src/jwks/rsa/insecure_test_jwk.json
 !types/src/jwks/rsa/secure_test_jwk.json
 !types/src/jwks/rsa/insecure_test_jwk_private_key.pem
+
+!oinori
+!oinori-configs
```

---

### `.gitignore`
- **変更箇所**: `perf.data`, `perf.data.old`, `single.svg`, `.direnv` を追加
- **目的**: perfプロファイリング結果ファイルとNix direnvのキャッシュディレクトリをGit管理から除外

```diff
@@ -135,3 +135,8 @@ test_indexer_grpc/*
 *.dot
 *.bytecode
 !third_party/move/move-prover/tests/xsources/design/*.bytecode
+
+perf.data
+perf.data.old
+single.svg
+.direnv
```

---

### `Cargo.toml`（ワークスペースルート）
- **変更箇所**:
  1. `members`に `"oinori/*"` を追加
  2. 軽微なフォーマット変更（空行追加）
- **目的**: oinoriクレートをCargoワークスペースのメンバーとして登録し、ワークスペース全体でビルド・依存関係解決できるようにする

```diff
@@ -251,8 +251,10 @@ members = [
     "tools/compute-module-expansion-size",
     "types",
     "vm-validator",
+    "oinori/*",
 ]

+
 # NOTE: default-members is the complete list of binaries that form the "production Aptos codebase".
@@ -796,6 +798,7 @@ ureq = { version = "1.5.4", features = [
 ], default_features = false }
 url = { version = "2.4.0", features = ["serde"] }
+
 uuid = { version = "1.0.0", features = ["v4", "serde"] }
```

---

### `README.md`
- **変更箇所**: 元のAptos公式READMEを完全に置き換え
- **目的**: oinori-mevプロジェクトとしてのREADMEに更新
- **内容（英語・日本語両方）**:
  - aptos-coreのフォークであること
  - 概要：Aptosブロックチェーン上のアトミックアービトラージ（MEV）エンジン
  - 免責事項：動作保証なし、学習目的のみ、金銭的責任の否認、投資助言ではない

```diff
-<a href="https://aptos.dev">
-	<img width="100%" src="./.assets/aptos_banner.png" alt="Aptos Banner" />
-</a>
-
----
-
-[![License](https://img.shields.io/badge/license-Apache-green.svg)](LICENSE)
-...
-
-Aptos is a layer 1 blockchain bringing a paradigm shift to Web3 through better technology...
+# Oinori MEV - Atomic Arbitrage on Aptos
+
+This repository is a fork of [aptos-core](https://github.com/aptos-labs/aptos-core) with an added
+atomic arbitrage (MEV) engine called **oinori-mev**.
+
+## Overview
+
+Oinori-mev is an implementation of atomic arbitrage functionality on the Aptos blockchain.
+It monitors on-chain transactions and executes arbitrage opportunities across decentralized
+exchanges (DEXs).
+
+## Disclaimer
+
+### No Warranty
+This code was confirmed to be operational at past commit points.
+**There is no guarantee that it works in its current state.**
+
+### Educational Purpose Only
+**This is educational code only.**
+
+### No Financial Responsibility
+**The creator(s) of this code accept absolutely no responsibility for any financial losses.**
+
+### Not Investment Advice
+**This code and repository do not constitute investment advice.**
+
+---
+
+# Oinori MEV - Aptos上のアトミックアービトラージ（日本語）
+
+（上記の日本語版が続く）
```
