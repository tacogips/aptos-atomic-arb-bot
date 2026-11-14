# oinori-mev transaction module

Responsible for transaction construction, signing, and submission. Provides the flow from building Move scripts/entry functions to mempool submission.

---

## `oinori/oinori-mev/src/transaction/mod.rs`
- **Purpose**: Type definitions and module exports related to transactions

### `TransactionBase` structure
- `script_call: ScriptCall` — compiled script bytecode
- `max_gas: u64` — maximum gas amount

### `TransactionSender` structure
- Holds the sender address, private key, and chain ID
- `build_signed_tx()`: Builds a `SignedTransaction` from the payload, sequence number, and gas settings

---

## `oinori/oinori-mev/src/transaction/sender/mod.rs`
- **Purpose**: Transaction submission interface
- Direct submission to the internal mempool (submission to the node's internal mempool rather than through an external RPC)

---

## `oinori/oinori-mev/src/transaction/builder/entry_function.rs`
- **Purpose**: Build payloads for Move entry function calls

### `EntryFunctionCall::as_transaction_payload()`
- **Purpose**: Convert an entry function call into a `TransactionPayload`
- **Logic**:
  1. Convert the `FunctionArg` list into a BCS-serialized byte array
  2. Convert the `FunctionTypeTag` list into Move `TypeTag`s
  3. Build `TransactionPayload::EntryFunction`

---

## `oinori/oinori-mev/src/transaction/builder/function_args.rs`
- **Purpose**: Type conversion for Move function arguments

### `FunctionArg::as_transaction_arg()`
- **Purpose**: Convert each type (U8, U64, U128, Bool, Address, String, Vector) into transaction arguments
- **Logic**: Parse according to the type and map into the `TransactionArgument` enum

### `as_type_and_value()`
- **Purpose**: Generate a pair of a type tag and BCS-encoded value
- **Logic**: Use the Move type parser to generate a `TypeTag` and byte array from a string

---

## `oinori/oinori-mev/src/transaction/builder/package_builder.rs`
- **Purpose**: Build and deploy Move packages

### `BuildPackageInfo::build_package()`
- **Purpose**: Compile a Move package
- **Logic**:
  1. Validate the presence of `Move.toml`
  2. Configure build options (`compact` / `fat`)
  3. Run compilation with `BuiltPackage::build()`

### `build_module_publish_transaction()`
- **Purpose**: Build a transaction payload for module publishing
- **Logic**: Combine compiled code and metadata with `code_publish_package_txn`

---

## `oinori/oinori-mev/src/transaction/builder/script.rs`
- **Purpose**: Compile Move scripts and generate payloads

### `ScriptCall::build_script()`
- **Purpose**: Extract script bytecode from a Move package
- **Constraint**: The package must contain exactly one script

### `ScriptCall::as_transaction_payload()`
- **Purpose**: Build a `Script`-type `TransactionPayload` with arguments and type parameters

---

## `oinori/oinori-mev/src/transaction/builder/type_tag.rs`
- **Purpose**: Convert type parameters when invoking swap contracts

### `FunctionTypeTag` enum
- `Coin(CoinId)` -> StructTag for a coin
- `CurveType(Option<CurveTypeTag>)` -> Liquid stable/uncorrelated curve type

### `convert_swap_curve_type_tag()`
- **Purpose**: Convert from swap contract and AMM curve type to `CurveTypeTag`
- **Logic**: For the Liquid protocol only, return `LiquidStable` or `LiquidUncorrelated`
