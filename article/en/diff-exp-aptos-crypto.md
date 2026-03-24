# Changes to the `crates/aptos-crypto` package

Fixes to cryptographic key types. Makes the x25519 private key needed for MEV engine network connections clonable.

---

## `crates/aptos-crypto/src/x25519.rs`

### Change: added `Clone` derive to `PrivateKey`
- **Before**: `#[derive(DeserializeKey, SilentDisplay, SilentDebug, SerializeKey)]` + `#[cfg_attr(any(test, feature = "fuzzing"), derive(Clone))]`
- **After**: `#[derive(DeserializeKey, SilentDisplay, SilentDebug, SerializeKey, Clone)]` (removes the test/fuzzing condition and makes it always clonable)
- **Purpose**: Allow the x25519 private key to be cloned at runtime
- **Background**: oinori-mev needs to generate a random private key when connecting to peers and pass it across multiple function calls. The original design only allowed cloning the private key in tests and fuzzing, but production code also needed it, so it was changed to always be clonable
- **Security note**: Cloning a private key is normally restricted for security reasons, because it increases the risk of multiple copies existing in memory. In oinori, a new random key is generated for each peer connection, so each key is temporary and the risk of long-term key material leakage is limited

```diff
@@ -64,8 +64,9 @@ pub const SHARED_SECRET_SIZE: usize = 32;

 /// This type should be used to deserialize a received private key
-#[derive(DeserializeKey, SilentDisplay, SilentDebug, SerializeKey)]
-#[cfg_attr(any(test, feature = "fuzzing"), derive(Clone))]
+// oinori: added Clone derive
+#[derive(DeserializeKey, SilentDisplay, SilentDebug, SerializeKey, Clone)]
+//#[cfg_attr(any(test, feature = "fuzzing"), derive(Clone))]
 pub struct PrivateKey(x25519_dalek::StaticSecret);
```
