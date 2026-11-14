# Project Instructions

## Git Diff Policy

- When comparing against `main` branch, always exclude `Cargo.lock` from the diff:
  ```
  git diff main -- ':!Cargo.lock'
  ```
- Cargo.lock is large and contains many dependency changes that are not relevant to code review

## Project Goal

The purpose of modifications on this branch is to prepare the codebase for public release as an **educational material for atomic arbitrage**.

### Objectives

1. Remove unnecessary items from the diff between `feature/to_book` and `main`
2. Clean up the code to be suitable as a general atomic arb tutorial/reference
3. Ensure no sensitive information remains in the code

### Cleanup Checklist

- [ ] Remove all credential information (private keys, API keys, tokens)
- [ ] Remove hardcoded addresses that are not publicly known
- [ ] Remove unnecessary debug code and commented-out code
- [ ] Remove environment-specific configurations
- [ ] Keep only essential code for demonstrating atomic arbitrage concepts

## Article Directory (`article/`)

`article/` ディレクトリには、`main`ブランチと`feature/to_book`ブランチ間のdiffを、パッケージ/ファイル単位で分析・解説した日本語のMarkdownファイルが格納されている。これらはoinori-mevの技術記事を執筆するための資料である。

### 目的

- aptos-coreフォークにおける各変更箇所の**目的と意図**を日本語で記録する
- 各ファイルに**実際のdiffコード**を埋め込み、変更内容を具体的に示す
- oinoriパッケージ（`oinori/`配下）自体は対象外とし、aptos-core側の変更のみを対象とする

### ファイル命名規則

`diff-exp-{パッケージ名またはカテゴリ名}.md`

### ファイル一覧

| ファイル | 対象 |
|---|---|
| `diff-exp-root-configs.md` | ルート設定ファイル（.dockerignore, .envrc, .gitignore, Cargo.toml, README.md, flake.nix等） |
| `diff-exp-api.md` | `api/` パッケージ（API型の公開、ブロードキャスト先指定） |
| `diff-exp-aptos-node.md` | `aptos-node/` パッケージ（MEVランタイム統合、ノード起動フロー） |
| `diff-exp-aptos-crypto.md` | `crates/aptos-crypto/`（x25519 PrivateKeyのClone対応） |
| `diff-exp-docker.md` | `docker/` ビルドファイル（oinori用Dockerfile） |
| `diff-exp-mempool.md` | `mempool/` パッケージ（バリデーション省略、即時ブロードキャスト） |
| `diff-exp-network-framework.md` | `network/framework/`（接続ID回転、地域フィルタ、Noiseカスタムidentity） |
| `diff-exp-network-netcore.md` | `network/netcore/`（Transportトレイトのdial署名変更） |
| `diff-exp-state-sync.md` | `state-sync/`（ピア選択の簡素化、固定ピア数） |
| `diff-exp-types.md` | `types/`（NetworkAddressにfind_dns()追加） |
| `diff-exp-oinori-configs.md` | `oinori-configs/`（メインネットfullnode設定） |

### 記事ファイルの構成例

各ファイルは以下の構成に従う:

```markdown
# パッケージ名の変更

概要説明

---

## `対象ファイルパス`
- **変更箇所**: 変更の概要
- **目的**: なぜこの変更が必要か

\```diff
@@ -xx,x +xx,x @@
 既存コード
+追加コード
-削除コード
\```
```

## Disclaimer Requirements

When modifying README.md or any documentation, ensure the following disclaimers are always included (in both English and Japanese):

1. **Fork Origin**: This is a fork of aptos-core with oinori-mev atomic arbitrage functionality
2. **No Warranty**: Code worked at past commits but no guarantee for current state
3. **Educational Only**: This is learning/educational code only
4. **No Financial Responsibility**: Creator accepts no responsibility for financial losses
5. **Not Investment Advice**: Does not constitute investment advice, does not recommend investment
