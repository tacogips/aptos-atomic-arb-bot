# oinori-mev transaction モジュール

トランザクションの構築・署名・送信を担当する。Moveスクリプト/エントリーファンクションのビルドからmempool送信までのフローを提供する。

---

## `oinori/oinori-mev/src/transaction/mod.rs`
- **目的**: トランザクション関連の型定義とモジュールエクスポート

### `TransactionBase`構造体
- `script_call: ScriptCall` — コンパイル済みスクリプトバイトコード
- `max_gas: u64` — 最大ガス量

### `TransactionSender`構造体
- 送信者アドレス、秘密鍵、チェーンIDを保持
- `build_signed_tx()`: ペイロード、シーケンス番号、ガス設定からSignedTransactionを構築

---

## `oinori/oinori-mev/src/transaction/sender/mod.rs`
- **目的**: トランザクション送信インターフェース
- 内部mempoolへの直接送信（外部RPCではなくノード内部のmempoolに送信）

---

## `oinori/oinori-mev/src/transaction/builder/entry_function.rs`
- **目的**: Moveエントリーファンクション呼び出しのペイロード構築

### `EntryFunctionCall::as_transaction_payload()`
- **目的**: エントリーファンクション呼び出しをTransactionPayloadに変換
- **ロジック**:
  1. `FunctionArg`リストを`bcs`シリアライズ済みバイト列に変換
  2. `FunctionTypeTag`リストをMoveの`TypeTag`に変換
  3. `TransactionPayload::EntryFunction`を構築

---

## `oinori/oinori-mev/src/transaction/builder/function_args.rs`
- **目的**: Move関数引数の型変換

### `FunctionArg::as_transaction_arg()`
- **目的**: 各型（U8, U64, U128, Bool, Address, String, Vector）のトランザクション引数変換
- **ロジック**: 型に応じたパースと`TransactionArgument` enumへのマッピング

### `as_type_and_value()`
- **目的**: 型タグとBCSエンコード値のペアを生成
- **ロジック**: Move型パーサーを使用して文字列からTypeTagとバイト列を生成

---

## `oinori/oinori-mev/src/transaction/builder/package_builder.rs`
- **目的**: Moveパッケージのビルドとデプロイ

### `BuildPackageInfo::build_package()`
- **目的**: Moveパッケージをコンパイル
- **ロジック**:
  1. `Move.toml`の存在を検証
  2. ビルドオプション設定（compact/fat）
  3. `BuiltPackage::build()`でコンパイル実行

### `build_module_publish_transaction()`
- **目的**: モジュールパブリッシュ用トランザクションペイロードの構築
- **ロジック**: コンパイル済みコードとメタデータを`code_publish_package_txn`で結合

---

## `oinori/oinori-mev/src/transaction/builder/script.rs`
- **目的**: Moveスクリプトのコンパイルとペイロード生成

### `ScriptCall::build_script()`
- **目的**: Moveパッケージからスクリプトバイトコードを抽出
- **制約**: パッケージ内のスクリプトは正確に1つでなければならない

### `ScriptCall::as_transaction_payload()`
- **目的**: 引数と型パラメータを添えてScript型TransactionPayloadを構築

---

## `oinori/oinori-mev/src/transaction/builder/type_tag.rs`
- **目的**: スワップコントラクト呼び出し時の型パラメータ変換

### `FunctionTypeTag` enum
- `Coin(CoinId)` → コインのStructTag
- `CurveType(Option<CurveTypeTag>)` → LiquidのStable/Uncorrelatedカーブ型

### `convert_swap_curve_type_tag()`
- **目的**: スワップコントラクトとAMMカーブタイプからCurveTypeTagに変換
- **ロジック**: Liquidプロトコルの場合のみ、`LiquidStable`または`LiquidUncorrelated`を返す
