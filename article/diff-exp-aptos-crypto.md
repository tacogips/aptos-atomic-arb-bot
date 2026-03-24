# crates/aptos-crypto パッケージの変更

暗号鍵型の修正。MEVエンジンのネットワーク接続で必要なx25519秘密鍵のClone可能化。

---

## `crates/aptos-crypto/src/x25519.rs`

### 変更箇所: `PrivateKey`のClone derive追加
- **変更前**: `#[derive(DeserializeKey, SilentDisplay, SilentDebug, SerializeKey)]` + `#[cfg_attr(any(test, feature = "fuzzing"), derive(Clone))]`
- **変更後**: `#[derive(DeserializeKey, SilentDisplay, SilentDebug, SerializeKey, Clone)]`（テスト/fuzzing条件を撤廃して常にClone可能に）
- **目的**: x25519秘密鍵をランタイムでCloneできるようにする
- **背景**: oinori-mevはピアとの接続時にランダムな秘密鍵を生成し、複数の関数呼び出し間で渡す必要がある。元の設計では秘密鍵のCloneはテスト/fuzzing時のみ許可されていたが、本番コードでも必要になったため常時Clone可能に変更した
- **セキュリティ上の注意**: 秘密鍵のCloneは本来セキュリティ上の理由で制限されている（メモリ上に複数コピーが存在するリスク）。oinoriではピア接続のたびに新しいランダムキーを生成するため、各キーは一時的なものであり、長期的なキーマテリアルの漏洩リスクは限定的

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
