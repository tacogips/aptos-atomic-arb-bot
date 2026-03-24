# network/netcore パッケージの変更

ネットワークコアのトランスポート抽象化レイヤー。`Transport` traitの`dial`メソッドシグネチャを変更し、全トランスポート実装でカスタムアイデンティティを受け取れるようにする。

---

## `network/netcore/Cargo.toml`

### 変更箇所: aptos-crypto依存関係の追加
- **変更内容**: `aptos-crypto = { workspace = true }` を追加
- **目的**: `x25519::PrivateKey`型を使用するため

```diff
@@ -24,6 +24,8 @@ tokio = { workspace = true }
 tokio-util = { workspace = true }
 url = { workspace = true }

+aptos-crypto = { workspace = true }
+
 [dev-dependencies]
```

---

## `network/netcore/src/transport/mod.rs`

### 変更箇所: `Transport` traitの`dial`メソッドシグネチャ変更
- **目的**: トランスポートtraitレベルでカスタムアイデンティティをサポート

```diff
+use aptos_crypto::x25519;
+
@@ -110,7 +112,14 @@ pub trait Transport {

     /// Dials the given [`NetworkAddress`], returning a future for a pending outbound connection.
-    fn dial(&self, peer_id: PeerId, addr: NetworkAddress) -> Result<Self::Outbound, Self::Error>
+    /// oinori: modified to accept custom identity for dialing
+    fn dial(
+        &self,
+        self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Self::Outbound, Self::Error>
     where
         Self: Sized;
 }
```

---

## `network/netcore/src/transport/and_then.rs`

### 変更箇所: `AndThen`トランスポートの`dial`メソッドシグネチャ変更

```diff
+use aptos_crypto::x25519;
+
@@ -51,8 +53,19 @@ where
         Ok((listener, addr))
     }

-    fn dial(&self, peer_id: PeerId, addr: NetworkAddress) -> Result<Self::Outbound, Self::Error> {
-        let fut = self.transport.dial(peer_id, addr.clone())?;
+    fn dial(
+        &self,
+        self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Self::Outbound, Self::Error> {
+        let fut = self.transport.dial(
+            self_peer_private_key,
+            oinori_self_peer_id,
+            peer_id,
+            addr.clone(),
+        )?;
         let origin = ConnectionOrigin::Outbound;
         let f = self.function.clone();
```

---

## `network/netcore/src/transport/boxed.rs`

### 変更箇所: `BoxedTransport`の`dial`メソッドシグネチャ変更

```diff
+use aptos_crypto::x25519;
+
 trait AbstractBoxedTransport<O, E> {
     fn listen_on(&self, addr: NetworkAddress) -> Result<(Listener<O, E>, NetworkAddress), E>;
-    fn dial(&self, peer_id: PeerId, addr: NetworkAddress) -> Result<Outbound<O, E>, E>;
+
+    fn dial(
+        &self,
+        self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Outbound<O, E>, E>;
 }

@@ -35,8 +44,14 @@ where
         Ok((listener.boxed() as Listener<O, E>, addr))
     }

-    fn dial(&self, peer_id: PeerId, addr: NetworkAddress) -> Result<Outbound<O, E>, E> {
-        let outgoing = self.dial(peer_id, addr)?;
+    fn dial(
+        &self,
+        self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Outbound<O, E>, E> {
+        let outgoing = self.dial(self_peer_private_key, oinori_self_peer_id, peer_id, addr)?;
         Ok(outgoing.boxed() as Outbound<O, E>)
     }
 }
@@ -80,7 +95,14 @@ where
         self.inner.listen_on(addr)
     }

-    fn dial(&self, peer_id: PeerId, addr: NetworkAddress) -> Result<Self::Outbound, Self::Error> {
-        self.inner.dial(peer_id, addr)
+    fn dial(
+        &self,
+        self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Self::Outbound, Self::Error> {
+        self.inner
+            .dial(self_peer_private_key, oinori_self_peer_id, peer_id, addr)
     }
 }
```

---

## `network/netcore/src/transport/memory.rs`

### 変更箇所: `MemoryTransport`の`dial`メソッドシグネチャ変更

```diff
@@ -51,7 +51,12 @@ impl Transport for MemoryTransport {
         Ok((Listener::new(listener), listen_addr))
     }

-    fn dial(&self, _peer_id: PeerId, addr: NetworkAddress) -> Result<Self::Outbound, Self::Error> {
+    fn dial(
+        &self,
+        _oinori_self_peer_id: PeerId,
+        _peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Self::Outbound, Self::Error> {
```

テスト修正:

```diff
-        let outbound = t.dial(peer_id, addr)?;
+        let outbound = t.dial(PeerId::random(), peer_id, addr)?;

-        let result = t.dial(peer_id, "/ip4/127.0.0.1/tcp/22".parse().unwrap());
+        let result = t.dial(
+            PeerId::random(),
+            peer_id,
+            "/ip4/127.0.0.1/tcp/22".parse().unwrap(),
+        );
```

---

## `network/netcore/src/transport/tcp.rs`

### 変更箇所: `TcpTransport`の`dial`メソッドシグネチャ変更

```diff
+use aptos_crypto::x25519;
+
@@ -137,7 +139,13 @@ impl Transport for TcpTransport {
         ))
     }

-    fn dial(&self, _peer_id: PeerId, addr: NetworkAddress) -> Result<Self::Outbound, Self::Error> {
+    fn dial(
+        &self,
+        _self_peer_private_key: x25519::PrivateKey,
+        _oinori_self_peer_id: PeerId,
+        _peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<Self::Outbound, Self::Error> {
```

テスト修正:

```diff
-        let dial = t.dial(peer_id, addr)?;
+        let dial = t.dial(PeerId::random(), peer_id, addr)?;

-        let result = t.dial(peer_id, "/memory/22".parse().unwrap());
+        let result = t.dial(PeerId::random(), peer_id, "/memory/22".parse().unwrap());
```
