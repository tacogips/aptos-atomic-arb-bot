# Changes to the `api` package

Changes to aptos-core's REST API layer. This exposes API types needed for sending transactions from the MEV engine to the mempool and for parsing Move modules.

---

## `api/src/context.rs`

### Change: added an argument to `MempoolClientRequest::SubmitTransaction`
- **Change**: `SubmitTransaction(txn, req_sender)` -> `SubmitTransaction(txn, req_sender, None)`
- **Purpose**: This matches the third argument added to the `MempoolClientRequest::SubmitTransaction` enum, an optional list of peer IDs to broadcast to (`Option<Vec<PeerNetworkId>>`). For transaction submission via the REST API, direct broadcast to a specific peer is unnecessary, so `None` is passed
- **Background**: From the MEV engine, this option is used to immediately broadcast to a specific validator peer. Standard API-based submission continues to follow the normal broadcast flow

```diff
@@ -208,7 +208,9 @@ impl Context {
         let (req_sender, callback) = oneshot::channel();
         self.mp_sender
             .clone()
-            .send(MempoolClientRequest::SubmitTransaction(txn, req_sender))
+            .send(MempoolClientRequest::SubmitTransaction(
+                txn, req_sender, None,
+            ))
             .await?;
```

---

## `api/types/src/convert.rs`

### Change: made `explain_function_index` public
- **Change**: `fn explain_function_index` -> `pub fn explain_function_index`
- **Purpose**: Expose the method that resolves a function name from a Move VM entry function index. oinori-mev uses this when analyzing Move function calls inside transactions
- **Background**: In arbitrage, it is necessary to identify which DEX function a mempool transaction is calling, such as a swap function. Making this method public allows DEX swap information to be extracted from transaction payloads

```diff
@@ -1088,7 +1088,9 @@ impl<'a, S: StateView> MoveConverter<'a, S> {
         }
     }

-    fn explain_function_index(&self, module_id: &ModuleId, function: &u16) -> Result<String> {
+    //TODO(oinori )
+    //fn explain_function_index(&self, module_id: &ModuleId, function: &u16) -> Result<String> {
+    pub fn explain_function_index(&self, module_id: &ModuleId, function: &u16) -> Result<String> {
         let code = self.inner.view_existing_module(module_id)?;
         let func = code.function_handle_at(FunctionHandleIndex::new(*function));
```

---

## `api/types/src/lib.rs`

### Change: re-exported `MoveStructValue`
- **Change**: Added `pub use move_types::MoveStructValue;`
- **Purpose**: Make the Move VM structure value type (`MoveStructValue`) available from the oinori-mev crate
- **Background**: When reading DEX pool resources on-chain, such as liquidity pool balances, they need to be deserialized as Move structure values. Exporting this type directly from the api-types crate makes resource reading in oinori-mev simpler

```diff
@@ -60,6 +60,9 @@ pub use transaction::{
 pub use view::{ViewFunction, ViewRequest};
 pub use wrappers::{EventGuid, IdentifierWrapper, StateKeyWrapper};

+// oinori: export for mev
+pub use move_types::MoveStructValue;
+
 pub fn deserialize_from_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
```
