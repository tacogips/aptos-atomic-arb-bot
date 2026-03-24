# Changes to the network/framework package

Changes to Aptos's peer-to-peer networking framework. In oinori, this introduces a mechanism that periodically reconnects to validators with a new random ID, plus geographic filtering of connection targets.

### Connection pools and ID rotation - oinori's biggest idea

oinori communicates with validators using Aptos's custom protocol, but because of the nature of an MEV bot, **requests that include errors occur frequently** due to simulation failures and timing issues. Aptos validators have a mechanism that blocks connections from clients that send too many error-prone requests (rate limiting / banning).

oinori's solution to this problem is **periodically refreshing the connection ID in the connection pool**. Specifically:

1. **Generate a random ID**: For every connection, generate a new x25519 private key at random and connect with a new PeerID derived from it (`oinori_new_self_peer_id()`)
2. **Manage the connection pool**: Record the connection time for each validator in `oinori_peer_connected_log` and use it to manage the connection lifecycle
3. **Periodic ID rotation**: Use a timer that fires every 3 seconds (`oinori_reconnect_ticker`) to monitor connection state, disconnect connections that have been alive for more than 10 seconds, and **reconnect with a new random ID** (`oinori_reconnect_peers()`)
4. **Gradual rotation**: Limit the number of connections rotated at once to 10 (`update_peer_num_at_once = 10`) to keep the connection state stable

With this mechanism, the validator side always sees connections as coming from a different client, which avoids being banned as a specific client. It requires modifications across the entire Aptos-core networking stack, including `oinori_upgrade_outbound`, which uses a custom ID during the Noise protocol handshake, and adding a `self_peer_private_key` parameter throughout the Transport trait. This is **the most carefully engineered part of oinori's implementation**.

---

## `network/framework/src/connectivity_manager/mod.rs`

### Change 1: Add infrastructure for reconnecting peers with random IDs
- **Added imports**: `aptos_crypto::Uniform`, `from_identity_public_key`, and `rand`-related imports
- **New field**: `oinori_peer_connected_log: Arc<RwLock<HashMap<PeerId, SystemTime>>>`
- **Purpose**: Track the connection time for each peer and manage state for reconnecting with a new random ID after some time has passed

```diff
@@ -72,6 +72,15 @@ mod selection;
 #[cfg(test)]
 mod test;

+// oinori: imports for peer reconnection
+use aptos_crypto::Uniform;
+use aptos_types::account_address::from_identity_public_key;
+use rand::prelude::*;
+use rand::{
+    rngs::{OsRng, StdRng},
+    Rng as rand_Rng, SeedableRng,
+};
+
@@ -130,6 +139,9 @@ pub struct ConnectivityManager<TBackoff> {
     mutual_authentication: bool,
     enable_latency_aware_dialing: bool,
+
+    /// oinori: track peer connection times for reconnection logic
+    oinori_peer_connected_log: Arc<RwLock<HashMap<PeerId, SystemTime>>>,
 }
```

### Change 2: Add the `oinori_new_self_peer_id()` function

```diff
+fn oinori_new_self_peer_id() -> (x25519::PrivateKey, PeerId) {
+    let mut rng = StdRng::from_seed(OsRng.gen());
+    let key = x25519::PrivateKey::generate(&mut rng);
+    (key.clone(), from_identity_public_key(key.public_key()))
+}
```

### Change 3: Add a periodic reconnection timer and handler

```diff
@@ -415,6 +438,10 @@ where
         let ticker = self.time_service.interval(self.connectivity_check_interval);
         tokio::pin!(ticker);

+        // oinori: periodic reconnection ticker
+        let oinori_reconnect_ticker = self.time_service.interval(Duration::from_secs(3));
+        tokio::pin!(oinori_reconnect_ticker);
+
@@ -426,6 +453,13 @@ where
                 _ = ticker.select_next_some() => {
                     self.check_connectivity(&mut pending_dials).await;
                 },
+
+
+                // oinori: handle reconnection
+                _ = oinori_reconnect_ticker.select_next_some() => {
+                    self.oinori_reconnect_peers().await;
+                },
+
                 req = self.requests_rx.select_next_some() => {
```

### Change 4: Add the `oinori_reconnect_peers()` method

```diff
+    /// oinori: reconnect expired peers with new identity
+    async fn oinori_reconnect_peers(&mut self) {
+        let connection_reqs_tx = self.connection_reqs_tx.clone();
+        let oinori_peer_connected_log = self.oinori_peer_connected_log.clone();
+
+        let expire_duration = Duration::from_secs(10);
+        let update_peer_num_at_once = 10;
+
+        let reconnect_target_peers: Vec<PeerId> = {
+            let mut expired_peers: Vec<PeerId> = vec![];
+            for (peer_id, connected_at) in oinori_peer_connected_log.read().iter() {
+                if connected_at.elapsed().unwrap() > expire_duration {
+                    expired_peers.push(peer_id.clone());
+                }
+            }
+
+            expired_peers
+                .choose_multiple(&mut rand::thread_rng(), update_peer_num_at_once)
+                .cloned()
+                .collect::<Vec<PeerId>>()
+        };
+
+        for each_peer_id in reconnect_target_peers.into_iter() {
+            match connection_reqs_tx.disconnect_peer(each_peer_id).await {
+                Ok(_) => {
+                    let eligible_peers = self.discovered_peers.read().peer_set.clone();
+                    if let Some(discovered_peer) = eligible_peers.get(&each_peer_id) {
+                        let init_dial_state = DialState::new(self.backoff_strategy.clone());
+                        let dial_state = self.dial_states.entry(each_peer_id)
+                            .or_insert_with(|| init_dial_state);
+                        let peer_addr = dial_state.next_addr(&discovered_peer.addrs).clone();
+                        let peer_addr = match peer_addr {
+                            Some(peer_addr) => peer_addr,
+                            None => continue,
+                        };
+                        let (self_peer_private_key, self_peer_id) = oinori_new_self_peer_id();
+                        match connection_reqs_tx.dial_peer(
+                            self_peer_private_key, self_peer_id,
+                            each_peer_id, peer_addr.clone(),
+                        ).await {
+                            Ok(_) => {
+                                oinori_peer_connected_log.write()
+                                    .insert(each_peer_id.clone(), SystemTime::now());
+                                self.discovered_peers.write()
+                                    .update_last_dial_time(&each_peer_id)
+                            },
+                            Err(e) => { error!("reconnect peer : reconnect error {}", e); },
+                        }
+                    }
+                },
+                Err(e) => { error!("reconnect peer :disconnect error {}", e); },
+            }
+        }
+    }
```

### Change 5: Remove validator geographic filtering and connection count limits

```diff
@@ -573,7 +684,8 @@ where
         let role = self.network_context.role();
         let roles_to_dial = network_id.upstream_roles(&role);
         let discovered_peers = self.discovered_peers.read().peer_set.clone();
-        let eligible_peers: Vec<_> = discovered_peers
+
+        let mut eligible_peers: Vec<_> = discovered_peers
             .into_iter()
             .filter(|(peer_id, peer)| {
                 peer.is_eligible_to_be_dialed()
@@ -583,6 +695,9 @@ where
             })
             .collect();

+        // oinori: filter and sort peers by location preference
+        oinori_filter_sort_eligible_peers(&mut eligible_peers);
+
@@ -590,57 +705,8 @@ where
                 .or_insert_with(|| DialState::new(self.backoff_strategy.clone()));
         }

-        // Limit the number of dialed connections from a fullnode.
-        let num_eligible_peers = eligible_peers.len();
-        let num_peers_to_dial = if let Some(outbound_connection_limit) = self.outbound_connection_limit {
-            // ... (~30 lines of connection limit logic) ...
-        } else {
-            num_eligible_peers
-        };
-
-        if num_peers_to_dial == 0 { return vec![]; }
-
-        // Prioritize the eligible peers and select the peers to dial
-        if selection::should_select_peers_by_latency(...) {
-            // ... latency-based selection ...
-        } else {
-            selection::choose_peers_to_dial_randomly(eligible_peers, num_peers_to_dial)
-        }
+        // oinori: connect to all eligible peers
+        eligible_peers
     }
```

### Change 6: Use a new random ID when connecting to peers

```diff
@@ -779,8 +846,18 @@ where
                     peer_id.short_str(),
                     addr
                 );
-                    match connection_reqs_tx.dial_peer(peer_id, addr.clone()).await {
-                        Ok(_) => DialResult::Success,
+
+                    // oinori: use new random identity for each connection
+                    let (self_peer_private_key, self_peer_id) = oinori_new_self_peer_id();
+                    match connection_reqs_tx.dial_peer(
+                        self_peer_private_key, self_peer_id,
+                        peer_id.clone(), addr.clone()).await {
+                        Ok(_) => {
+                            let mut oinori_peer_connected_log_writer = oinori_peer_connected_log.write();
+                            oinori_peer_connected_log_writer.insert(peer_id, SystemTime::now());
+                            DialResult::Success
+                        },
                         Err(e) => DialResult::Failed(e),
```

### Change 7: Add `oinori_filter_sort_eligible_peers()` and regional validator lists
- **Content**: Filter validators according to the value of the `KOREAN_VALIDATOR` environment variable, using either the Korea/Japan region list or the Europe region list

```diff
+/// oinori: filter and sort peers by colocation (korea or eu)
+fn oinori_filter_sort_eligible_peers(eligible: &mut Vec<(PeerId, DiscoveredPeer)>) {
+    if eligible.len() == 1 { return; } // seed
+
+    let validators_filter = if std::env::var("KOREAN_VALIDATOR")
+        .unwrap_or_else(|_| "0".to_string()).as_str() == "1"
+    {
+        korean_validators()
+    } else {
+        europe_validators()
+    };
+
+    let mut i = 0;
+    while i < eligible.len() {
+        if let Some((peer_id, _)) = eligible.get(i) {
+            if !validators_filter.contains(&peer_id.to_hex_literal()) {
+                let _ = eligible.remove(i);
+            } else {
+                i += 1;
+            }
+        } else { break; }
+    }
+}
+
+fn korean_validators() -> HashSet<String> {
+    // seoul/tokyo/yongsan-gu/singapore (~16 entries)
+    [ "0x4c893cf8d614f503e982df5b48e9443bab4ea6be4f922a5d3858305c0a3af3ee",
+      "0x99d623e94981c2a6d41306785d69d66c237c6d3ac305cb6f32307785fde7fda2",
+      // ... more entries ...
+    ].into_iter().map(|x| x.to_string()).collect::<HashSet<String>>()
+}
+
+fn europe_validators() -> HashSet<String> {
+    // Netherlands, France, Germany, Poland, Finland, Ireland, Sweden (~60 entries)
+    [ "0x324df1e27c4129a58d73851ae0e9366064dc666a73e747051e203694a4cb257",
+      "0x33f0c333f6fcb9d1ef54d54093f0482b31bb0bdcd34bfa446e7416ff930453c0",
+      // ... more entries ...
+    ].into_iter().map(|x| x.to_string()).collect::<HashSet<String>>()
+}
```

---

## `network/framework/src/noise/handshake.rs`

### Change: Add the `oinori_upgrade_outbound` method, about 88 lines
- **Content**: A custom version of the original `upgrade_outbound` method. It performs the Noise protocol handshake using the private key specified as the node's own ID

```diff
+    /// oinori: upgrade outbound connection with custom identity
+    pub async fn oinori_upgrade_outbound<TSocket, F>(
+        &self,
+        mut socket: TSocket,
+        self_peer_private_key: x25519::PrivateKey,
+        self_peer_id: PeerId,
+        remote_peer_id: PeerId,
+        remote_public_key: x25519::PublicKey,
+        time_provider: F,
+    ) -> Result<(NoiseStream<TSocket>, PeerRole), NoiseHandshakeError>
+    where
+        TSocket: AsyncRead + AsyncWrite + Debug + Unpin,
+        F: Fn() -> [u8; AntiReplayTimestamps::TIMESTAMP_SIZE],
+    {
+        let mut client_message = [0; Self::CLIENT_MESSAGE_SIZE];
+
+        // oinori: use custom self peer id
+        client_message[..PeerId::LENGTH].copy_from_slice(self_peer_id.as_ref());
+        client_message[PeerId::LENGTH..Self::PROLOGUE_SIZE]
+            .copy_from_slice(remote_public_key.as_slice());
+
+        let (prologue_msg, client_noise_msg) = client_message.split_at_mut(Self::PROLOGUE_SIZE);
+        let payload = time_provider();
+
+        let mut rng = rand::rngs::OsRng;
+        // oinori: use custom private key
+        let self_noise_config = noise::NoiseConfig::new(self_peer_private_key);
+
+        let initiator_state = self_noise_config
+            .initiate_connection(&mut rng, prologue_msg, remote_public_key, Some(&payload), client_noise_msg)
+            .map_err(NoiseHandshakeError::BuildClientHandshakeMessageFailed)?;
+
+        // send first handshake message
+        socket.write_all(&client_message).await.map_err(NoiseHandshakeError::ClientWriteFailed)?;
+        socket.flush().await.map_err(NoiseHandshakeError::ClientFlushFailed)?;
+
+        // receive server's response
+        let mut server_response = [0u8; Self::SERVER_MESSAGE_SIZE];
+        socket.read_exact(&mut server_response).await.map_err(NoiseHandshakeError::ClientReadFailed)?;
+
+        // parse and finalize
+        let (_, session) = self_noise_config
+            .finalize_connection(initiator_state, &server_response)
+            .map_err(NoiseHandshakeError::ClientFinalizeFailed)?;
+
+        let noise_stream = NoiseStream::new(socket, session);
+        let peer_role = self.extract_peer_role_from_trusted_peers(remote_peer_id);
+        Ok((noise_stream, peer_role))
+    }
```

---

## `network/framework/src/peer/mod.rs`

### Change 1: Remove trace logging in two places

```diff
@@ -194,13 +194,7 @@ where
     pub async fn start(mut self) {
         let remote_peer_id = self.remote_peer_id();
-        trace!(
-            NetworkSchema::new(&self.network_context)
-                .connection_metadata(&self.connection_metadata),
-            "{} Starting Peer actor for peer: {}",
-            self.network_context,
-            remote_peer_id.short_str()
-        );
+        // oinori: skip trace logging for performance
```

### Change 2: Add traffic metrics for RPC sends

```diff
@@ -635,6 +623,12 @@ where
             PeerRequest::SendRpc(request) => {
                 let protocol_id = request.protocol_id;
+
+                network_application_outbound_traffic(
+                    self.network_context,
+                    protocol_id,
+                    request.data.len() as u64,
+                );
                 if let Err(e) = self
```

---

## `network/framework/src/peer_manager/mod.rs`

### Change: Extend the fields in `ConnectionRequest::DialPeer`

```diff
@@ -430,7 +430,13 @@ where
         self.sample_connected_peers();
         match request {
-            ConnectionRequest::DialPeer(requested_peer_id, addr, response_tx) => {
+            ConnectionRequest::DialPeer(
+                oinori_self_peer_key,
+                oinori_self_peer_id,
+                requested_peer_id,
+                addr,
+                response_tx,
+            ) => {
```

---

## `network/framework/src/peer_manager/senders.rs`

### Change: Update the `dial_peer` method signature

```diff
@@ -116,12 +121,22 @@ impl ConnectionRequestSender {
     pub async fn dial_peer(
         &self,
+        self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
         peer: PeerId,
         addr: NetworkAddress,
     ) -> Result<(), PeerManagerError> {
         let (oneshot_tx, oneshot_rx) = oneshot::channel();
-        self.inner
-            .push(peer, ConnectionRequest::DialPeer(peer, addr, oneshot_tx))?;
+        self.inner.push(
+            peer,
+            ConnectionRequest::DialPeer(
+                self_peer_private_key,
+                oinori_self_peer_id,
+                peer,
+                addr,
+                oneshot_tx,
+            ),
+        )?;
         oneshot_rx.await?
     }
```

---

## `network/framework/src/peer_manager/transport.rs`

### Change: Extend the fields in `TransportRequest::DialPeer`

```diff
@@ -24,9 +24,13 @@ use std::time::Instant;
+use aptos_crypto::x25519;
+
 #[derive(Debug)]
 pub enum TransportRequest {
     DialPeer(
+        x25519::PrivateKey,
+        PeerId,
         PeerId,
         NetworkAddress,
         oneshot::Sender<Result<(), PeerManagerError>>,
```

---

## `network/framework/src/peer_manager/types.rs`

### Change: Update the type definition for `ConnectionRequest::DialPeer`

```diff
@@ -25,6 +27,8 @@ pub enum PeerManagerRequest {
 #[derive(Debug, Serialize)]
 pub enum ConnectionRequest {
     DialPeer(
+        x25519::PrivateKey,
+        PeerId,
         PeerId,
         NetworkAddress,
         #[serde(skip)] oneshot::Sender<Result<(), PeerManagerError>>,
```

---

## `network/framework/src/protocols/network/mod.rs`

### Change: Update the `NetworkSender::dial_peer` signature

```diff
@@ -370,8 +372,16 @@ impl<TMessage> NewNetworkSender for NetworkSender<TMessage> {
 impl<TMessage> NetworkSender<TMessage> {
-    pub async fn dial_peer(&self, peer: PeerId, addr: NetworkAddress) -> Result<(), NetworkError> {
-        self.connection_reqs_tx.dial_peer(peer, addr).await?;
+    pub async fn dial_peer(
+        &self,
+        oinori_self_peer_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer: PeerId,
+        addr: NetworkAddress,
+    ) -> Result<(), NetworkError> {
+        self.connection_reqs_tx
+            .dial_peer(oinori_self_peer_key, oinori_self_peer_id, peer, addr)
+            .await?;
         Ok(())
     }
```

---

## `network/framework/src/protocols/rpc/mod.rs`

### Change: Skip trace logging and metrics updates

```diff
@@ -477,14 +477,7 @@ impl OutboundRpcs {
         let request_id = self.request_id_gen.next();

-        trace!(
-            NetworkSchema::new(network_context).remote_peer(peer_id),
-            "{} Sending outbound rpc request with request_id {} and protocol_id {} to {}",
-            network_context,
-            request_id,
-            protocol_id,
-            peer_id.short_str(),
-        );
+        // oinori: skip trace logging for performance

@@ -499,8 +492,7 @@ impl OutboundRpcs {
         write_reqs_tx.push((), message)?;

-        // Update the outbound RPC request metrics
-        self.update_outbound_rpc_request_metrics(protocol_id, req_len);
+        // oinori: skip metrics update for performance
```

---

## `network/framework/src/transport/mod.rs`

### Change 1: Add the `oinori_upgrade_outbound` function, about 80 lines

```diff
+/// oinori: upgrade outbound connection with custom identity
+pub async fn oinori_upgrade_outbound<T: TSocket>(
+    ctxt: Arc<UpgradeContext>,
+    fut_socket: impl Future<Output = io::Result<T>>,
+    self_peer_private_key: x25519::PrivateKey,
+    self_peer_id: PeerId,
+    addr: NetworkAddress,
+    remote_peer_id: PeerId,
+    remote_pubkey: x25519::PublicKey,
+) -> io::Result<Connection<NoiseStream<T>>> {
+    let origin = ConnectionOrigin::Outbound;
+    let socket = fut_socket.await?;
+
+    let (mut socket, peer_role) = ctxt
+        .noise
+        .oinori_upgrade_outbound(
+            socket, self_peer_private_key, self_peer_id,
+            remote_peer_id.clone(), remote_pubkey,
+            AntiReplayTimestamps::now,
+        ).await.map_err(|err| { io::Error::new(io::ErrorKind::Other, err) })?;
```

### Change 2: Update the `AptosNetTransport::dial` signature

```diff
@@ -541,6 +623,8 @@ where
     pub fn dial(
         &self,
+        oinori_self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
         peer_id: PeerId,
         addr: NetworkAddress,
@@ -562,10 +646,24 @@ where

-        let fut_socket = self.base_transport.dial(peer_id, base_addr)?;
+        let fut_socket = self.base_transport.dial(
+            oinori_self_peer_private_key.clone(),
+            oinori_self_peer_id, peer_id, base_addr,
+        )?;
+
+        // oinori: use custom identity for outbound dial upgrade
+        let upgrade_fut = oinori_upgrade_outbound(
+            self.ctxt.clone(), fut_socket,
+            oinori_self_peer_private_key, oinori_self_peer_id,
+            addr, peer_id, pubkey,
+        );

-        let upgrade_fut = upgrade_outbound(self.ctxt.clone(), fut_socket, addr, peer_id, pubkey);
         let upgrade_fut = timeout_io(self.time_service.clone(), TRANSPORT_TIMEOUT, upgrade_fut);
         Ok(upgrade_fut)
     }
```

### Change 3: Update the `Transport` trait `dial` signature

```diff
@@ -653,9 +752,20 @@ where
-    fn dial(&self, peer_id: PeerId, addr: NetworkAddress) -> io::Result<Self::Outbound> {
-        self.dial(peer_id, addr)
-            .map(|upgrade_fut| upgrade_fut.boxed())
+    fn dial(
+        &self,
+        oinori_self_peer_private_key: x25519::PrivateKey,
+        oinori_self_peer_id: PeerId,
+        peer_id: PeerId,
+        addr: NetworkAddress,
+    ) -> io::Result<Self::Outbound> {
+        self.dial(
+            oinori_self_peer_private_key, oinori_self_peer_id,
+            peer_id, addr,
+        )
+        .map(|upgrade_fut| upgrade_fut.boxed())
     }
```
