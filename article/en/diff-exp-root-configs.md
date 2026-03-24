# Changes to Root Directory Configuration Files

Configuration files added or modified at the project root to run the oinori-mev atomic arbitrage engine on an aptos-core fork.

---

## Newly Added Files

### `.envrc` (newly added)
- **Purpose**: Automatically load the Nix development environment
- **Content**: `use flake .` — Automatically activates the development shell defined in `flake.nix` when entering the directory (direnv integration)

```diff
+use flake .
```

---

### `.taplo.toml` (newly added)
- **Purpose**: Formatter settings for TOML files
- **Content**: Excludes Cargo.lock, sets indentation to 4 spaces, and the column width to 80 characters. This is to keep formatting consistent when editing Cargo.toml.

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

### `CLAUDE.md` (newly added)
- **Purpose**: Project instruction file for Claude Code (AI assistant)
- **Content**:
  - Policy to exclude `Cargo.lock` when running `git diff main`
  - Clarifies that the project is being prepared for publication as educational material
  - Cleanup checklist (remove credentials, remove hardcoded addresses, etc.)
  - List of disclaimers that should be included in the README and similar docs

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

### `Taskfile.yml` (newly added)
- **Purpose**: Task runner definitions for Docker build/run via go-task
- **Content**:
  - `build`: Build the aptos-oinori image with Docker buildx
  - `run`: Start the oinori node in standard mode
  - `run-dry`: Start the node in dry-run mode (does not send real transactions)
  - `stop`: Stop the containers
  - `check`: Check that the node is running
  - `remove-lock-files`: Delete DB lock files after a crash

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

### `docker-compose.yml` (newly added)
- **Purpose**: Docker Compose definition for the oinori node
- **Content**:
  - `oinori`: Standard MEV node. Uses host network mode, mounts the data volume, and starts with `fullnode.yaml`
  - `oinori-dry-run`: Dry-run mode. Sets environment variable `OINORI_MEV_DRY_RUN=1` and does not send real transactions
  - Both containers expose ports 8080 (API), 9023 (Inspection), and 16182 (P2P)

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

### `flake.nix` (newly added)
- **Purpose**: Define a reproducible development environment with Nix Flakes
- **Content**:
  - Rust toolchain (read from `rust-toolchain.toml` via fenix)
  - Native build dependencies (`pkg-config`, `binutils`, `gcc`, `lld`, `openssl`, `zlib`, etc.)
  - Development tools (`go-task`, `nodejs`, etc.)
  - `LIBCLANG_PATH` and `LD_LIBRARY_PATH` settings
  - Automatic execution of `docker buildx use default`

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
+            export LD_LIBRARY_PATH="..."  # Set the paths for each library
+          '';
+        };
+      }
+    );
+}
```

`flake.lock` is the lock file that pins `flake.nix` dependencies (auto-generated).

---

## Modified Files

### `.dockerignore`
- **Changed**: Added `!oinori` and `!oinori-configs`
- **Purpose**: Ensure the oinori-related directories are included in the Docker build context. Since `.dockerignore` excludes everything by default, the required directories must be explicitly allowed.

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
- **Changed**: Added `perf.data`, `perf.data.old`, `single.svg`, and `.direnv`
- **Purpose**: Exclude perf profiling output files and the Nix direnv cache directory from Git tracking

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

### `Cargo.toml` (workspace root)
- **Changed**:
  1. Added `"oinori/*"` to `members`
  2. Minor formatting changes (added blank lines)
- **Purpose**: Register the oinori crate as a Cargo workspace member so the entire workspace can be built and dependencies resolved together

```diff
@@ -251,8 +251,10 @@ members = [
     "tools/compute-module-expansion-size",
     "types",
     "vm-validator",
+    "oinori/*",
 ]
+
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
- **Changed**: The original Aptos README was completely replaced
- **Purpose**: Update it to serve as the README for the oinori-mev project
- **Content (both English and Japanese)**:
  - That this is a fork of aptos-core
  - Overview: an atomic arbitrage (MEV) engine on the Aptos blockchain
  - Disclaimers: no warranty, educational use only, no financial responsibility, not investment advice

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
