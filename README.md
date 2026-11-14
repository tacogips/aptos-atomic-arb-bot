# Oinori MEV - Atomic Arbitrage on Aptos

This repository is a fork of [aptos-core](https://github.com/aptos-labs/aptos-core) with an added atomic arbitrage (MEV) engine called **oinori-mev**.
It is based on the upstream Aptos source as of 2024, specifically commit `28abb7db64af`, with additional modifications for this repository.

## Overview

Oinori-mev is an implementation of atomic arbitrage functionality on the Aptos blockchain. It monitors on-chain transactions and executes arbitrage opportunities across decentralized exchanges (DEXs).

## Disclaimer

### No Warranty

This code was confirmed to be operational at past commit points. **There is no guarantee that it works in its current state.** The Aptos blockchain, DEX protocols, and market conditions change constantly, and this code may be outdated or non-functional.

### Educational Purpose Only

**This is educational code only.** It is provided solely as technical learning material to understand:
- How atomic arbitrage works on blockchains
- MEV (Maximal Extractable Value) concepts
- Transaction construction and submission on Aptos
- DEX integration patterns

### No Financial Responsibility

**The creator(s) of this code accept absolutely no responsibility for any financial losses incurred through the use of this code.** By using this code, you acknowledge that:
- You are solely responsible for any losses
- Cryptocurrency trading and MEV activities carry significant financial risk
- You may lose all funds you use with this software

### Not Investment Advice

**This code and repository do not constitute investment advice.** This project:
- Does not recommend or encourage any investment activities
- Does not guarantee any profits or returns
- Should not be used as a basis for financial decisions

Use at your own risk.

---

## License

This project inherits the [Apache 2.0 License](LICENSE) from the original aptos-core repository.

---

# Oinori MEV - Aptos上のアトミックアービトラージ（日本語）

このリポジトリは [aptos-core](https://github.com/aptos-labs/aptos-core) からフォークし、**oinori-mev** と呼ばれるアトミックアービトラージ（MEV）エンジンを追加したものです。
本リポジトリは、2024年時点の Aptos 本家ソース、具体的には commit `28abb7db64af` をベースに変更を加えたものです。

## 概要

Oinori-mevは、Aptosブロックチェーン上でアトミックアービトラージ機能を実装したものです。オンチェーントランザクションを監視し、分散型取引所（DEX）間でアービトラージ機会を実行します。

## 免責事項

### 動作保証なし

このコードは過去のコミット時点では動作確認済みです。**現在の状態での動作は一切保証しません。** Aptosブロックチェーン、DEXプロトコル、市場状況は常に変化しており、このコードは古くなっているか、動作しない可能性があります。

### 学習目的のみ

**これは学習用コードです。** 以下を理解するための技術的な学習資料としてのみ提供されています：
- ブロックチェーン上でのアトミックアービトラージの仕組み
- MEV（Maximal Extractable Value）の概念
- Aptos上でのトランザクション構築と送信
- DEX統合パターン

### 金銭的責任の否認

**このコードの使用により発生したいかなる金銭的損失についても、コード作成者は一切の責任を負いません。** このコードを使用することにより、以下を了承したものとみなします：
- 損失はすべて自己責任です
- 暗号通貨取引およびMEV活動には重大な金銭的リスクがあります
- このソフトウェアで使用した資金をすべて失う可能性があります

### 投資助言ではありません

**このコードおよびリポジトリは投資助言を構成するものではありません。** このプロジェクトは：
- いかなる投資活動も推奨または奨励するものではありません
- いかなる利益やリターンも保証するものではありません
- 金銭的な意思決定の根拠として使用すべきではありません

すべて自己責任でご利用ください。

---

## Article Index / 記事一覧

The English articles in `article/en/` are the primary version. The Japanese articles in `article/ja/` are provided alongside them.

- Technical Techniques in oinori - Implementation Strategy for the Aptos MEV Bot ([EN](article/en/article-source-oinori-engineering-points.md) / [JA](article/ja/article-source-oinori-engineering-points.md))
- Changes to the `api` package ([EN](article/en/diff-exp-api.md) / [JA](article/ja/diff-exp-api.md))
- Changes to the `crates/aptos-crypto` package ([EN](article/en/diff-exp-aptos-crypto.md) / [JA](article/ja/diff-exp-aptos-crypto.md))
- Changes to the `aptos-node` package ([EN](article/en/diff-exp-aptos-node.md) / [JA](article/ja/diff-exp-aptos-node.md))
- Changes to docker-related files ([EN](article/en/diff-exp-docker.md) / [JA](article/ja/diff-exp-docker.md))
- Changes to the mempool package ([EN](article/en/diff-exp-mempool.md) / [JA](article/ja/diff-exp-mempool.md))
- Changes to the network/framework package ([EN](article/en/diff-exp-network-framework.md) / [JA](article/ja/diff-exp-network-framework.md))
- Changes to the network/netcore package ([EN](article/en/diff-exp-network-netcore.md) / [JA](article/ja/diff-exp-network-netcore.md))
- Changes in the `oinori-configs` Directory (Newly Added) ([EN](article/en/diff-exp-oinori-configs.md) / [JA](article/ja/diff-exp-oinori-configs.md))
- oinori-mev aggregator module ([EN](article/en/diff-exp-oinori-main-aggregator.md) / [JA](article/ja/diff-exp-oinori-main-aggregator.md))
- oinori-mev coins module ([EN](article/en/diff-exp-oinori-main-coins.md) / [JA](article/ja/diff-exp-oinori-main-coins.md))
- oinori-mev engine/route_filter module ([EN](article/en/diff-exp-oinori-main-engine-route-filter.md) / [JA](article/ja/diff-exp-oinori-main-engine-route-filter.md))
- oinori-mev engine module ([EN](article/en/diff-exp-oinori-main-engine.md) / [JA](article/ja/diff-exp-oinori-main-engine.md))
- oinori-mev lib.rs / entry point ([EN](article/en/diff-exp-oinori-main-lib.md) / [JA](article/ja/diff-exp-oinori-main-lib.md))
- oinori-mev read_db module ([EN](article/en/diff-exp-oinori-main-read-db.md) / [JA](article/ja/diff-exp-oinori-main-read-db.md))
- oinori-mev swap/amm module ([EN](article/en/diff-exp-oinori-main-swap-amm.md) / [JA](article/ja/diff-exp-oinori-main-swap-amm.md))
- oinori-mev swap/router module ([EN](article/en/diff-exp-oinori-main-swap-router.md) / [JA](article/ja/diff-exp-oinori-main-swap-router.md))
- oinori-mev swap/specifics module ([EN](article/en/diff-exp-oinori-main-swap-specifics.md) / [JA](article/ja/diff-exp-oinori-main-swap-specifics.md))
- oinori-mev transaction_stream module ([EN](article/en/diff-exp-oinori-main-transaction-stream.md) / [JA](article/ja/diff-exp-oinori-main-transaction-stream.md))
- oinori-mev transaction module ([EN](article/en/diff-exp-oinori-main-transaction.md) / [JA](article/ja/diff-exp-oinori-main-transaction.md))
- oinori-mev Utility Modules ([EN](article/en/diff-exp-oinori-main-utils.md) / [JA](article/ja/diff-exp-oinori-main-utils.md))
- Changes to Root Directory Configuration Files ([EN](article/en/diff-exp-root-configs.md) / [JA](article/ja/diff-exp-root-configs.md))
- Changes to the `state-sync` Package ([EN](article/en/diff-exp-state-sync.md) / [JA](article/ja/diff-exp-state-sync.md))
- Changes to the `types` Package ([EN](article/en/diff-exp-types.md) / [JA](article/ja/diff-exp-types.md))
