// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

//! The ConnectivityManager actor is responsible for ensuring that we are
//! connected to a node if and only if it is an eligible node.
//!
//! A list of eligible nodes is received at initialization, and updates are
//! received on changes to system membership. In our current system design, the
//! Consensus actor informs the ConnectivityManager of eligible nodes.
//!
//! Different discovery sources notify the ConnectivityManager of updates to
//! peers' addresses. Currently, there are 2 discovery sources (ordered by
//! decreasing dial priority, i.e., first is highest priority):
//!
//! 1. Onchain discovery protocol
//! 2. Seed peers from config
//!
//! In other words, if a we have some addresses discovered via onchain discovery
//! and some seed addresses from our local config, we will try the onchain
//! discovery addresses first and the local seed addresses after.
//!
//! When dialing a peer with a given list of addresses, we attempt each address
//! in order with a capped exponential backoff delay until we eventually connect
//! to the peer. The backoff is capped since, for validators specifically, it is
//! absolutely important that we maintain connectivity with all peers and heal
//! any partitions asap, as we aren't currently gossiping consensus messages or
//! using a relay protocol.

use crate::{
    application::storage::PeersAndMetadata,
    counters,
    logging::NetworkSchema,
    peer_manager::{self, conn_notifs_channel, ConnectionRequestSender, PeerManagerError},
    transport::ConnectionMetadata,
};
use aptos_config::{
    config::{Peer, PeerRole, PeerSet},
    network_id::NetworkContext,
};
use aptos_crypto::x25519;
use aptos_infallible::RwLock;
use aptos_logger::prelude::*;
use aptos_netcore::transport::ConnectionOrigin;
use aptos_num_variants::NumVariants;
use aptos_short_hex_str::AsShortHexStr;
use aptos_time_service::{TimeService, TimeServiceTrait};
use aptos_types::{account_address::AccountAddress, network_address::NetworkAddress, PeerId};
use futures::{
    channel::oneshot,
    future::{BoxFuture, FutureExt},
    stream::{FuturesUnordered, StreamExt},
};
use futures_util::future::join_all;
use itertools::Itertools;
use ordered_float::OrderedFloat;
use rand_latest::Rng;
use serde::Serialize;
use std::{
    cmp::{min, Ordering},
    collections::{hash_map::Entry, HashMap, HashSet},
    fmt,
    net::{Shutdown, TcpStream, ToSocketAddrs},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::task::JoinHandle;
use tokio_retry::strategy::jitter;

pub mod builder;
mod selection;
#[cfg(test)]
mod test;

// oinori: Connection Pool Identity Rotation
//
// Aptos validators rate-limit or ban clients that send excessive erroneous requests
// over their proprietary protocol. Since MEV bots inevitably generate many failed
// simulation/transaction requests, a single persistent identity would quickly get blocked.
//
// To work around this, oinori maintains a connection pool where each connection uses a
// randomly generated x25519 keypair (and thus a unique PeerId). Connections are periodically
// rotated with fresh identities so the validator sees each reconnection as a new client.
// This required changes across the entire network stack: Transport trait, Noise handshake,
// PeerManager, and ConnectivityManager.
use aptos_crypto::Uniform;
use aptos_types::account_address::from_identity_public_key;
use rand::prelude::*;
use rand::{
    rngs::{OsRng, StdRng},
    Rng as rand_Rng, SeedableRng,
};

/// In addition to the backoff strategy, we also add some small random jitter to
/// the delay before each dial. This jitter helps reduce the probability of
/// simultaneous dials, especially in non-production environments where most nodes
/// are spun up around the same time. Similarly, it smears the dials out in time
/// to avoid spiky load / thundering herd issues where all dial requests happen
/// around the same time at startup.
const MAX_CONNECTION_DELAY_JITTER: Duration = Duration::from_millis(100);

/// The maximum amount of time to wait before timing out a connection attempt.
/// This should be relatively small to avoid blocking dials for too long.
const MAX_CONNECTION_TIMEOUT_SECS: u64 = 2;

/// The maximum number of socket addresses to ping for a single address
const MAX_SOCKET_ADDRESSES_TO_PING: usize = 2;

/// The amount of time to try other peers until dialing this peer again.
///
/// It's currently set to 5 minutes to ensure rotation through all (or most) peers
const TRY_DIAL_BACKOFF_TIME: Duration = Duration::from_secs(300);

/// The ConnectivityManager actor.
pub struct ConnectivityManager<TBackoff> {
    network_context: NetworkContext,
    /// A handle to a time service for easily mocking time-related operations.
    time_service: TimeService,
    /// Peers and metadata
    peers_and_metadata: Arc<PeersAndMetadata>,
    /// PeerId and address of remote peers to which this peer is connected.
    connected: HashMap<PeerId, ConnectionMetadata>,
    /// All information about peers from discovery sources.
    discovered_peers: Arc<RwLock<DiscoveredPeerSet>>,
    /// Channel to send connection requests to PeerManager.
    connection_reqs_tx: ConnectionRequestSender,
    /// Channel to receive notifications from PeerManager.
    connection_notifs_rx: conn_notifs_channel::Receiver,
    /// Channel over which we receive requests from other actors.
    requests_rx: aptos_channels::Receiver<ConnectivityRequest>,
    /// Peers queued to be dialed, potentially with some delay. The dial can be canceled by
    /// sending over (or dropping) the associated oneshot sender.
    dial_queue: HashMap<PeerId, oneshot::Sender<()>>,
    /// The state of any currently executing dials. Used to keep track of what
    /// the next dial delay and dial address should be for a given peer.
    dial_states: HashMap<PeerId, DialState<TBackoff>>,
    /// Trigger connectivity checks every interval.
    connectivity_check_interval: Duration,
    /// Backoff strategy.
    backoff_strategy: TBackoff,
    /// Maximum delay b/w 2 consecutive attempts to connect with a disconnected peer.
    max_delay: Duration,
    /// A local counter incremented on receiving an incoming message. Printing this in debugging
    /// allows for easy debugging.
    event_id: u32,
    /// A way to limit the number of connected peers by outgoing dials.
    outbound_connection_limit: Option<usize>,
    /// Whether we are using mutual authentication or not
    mutual_authentication: bool,
    /// Whether or not to enable latency aware peer dialing
    enable_latency_aware_dialing: bool,

    /// oinori: Tracks when each peer connection was established.
    /// Used to determine which connections have exceeded the TTL (`expire_duration`)
    /// and need to be rotated with a fresh random identity.
    oinori_peer_connected_log: Arc<RwLock<HashMap<PeerId, SystemTime>>>,
}

/// Different sources for peer addresses, ordered by priority (Onchain=highest,
/// Config=lowest).
#[repr(u8)]
#[derive(Copy, Clone, Eq, Hash, PartialEq, Ord, PartialOrd, NumVariants, Serialize)]
pub enum DiscoverySource {
    OnChainValidatorSet,
    File,
    Rest,
    Config,
}

impl fmt::Debug for DiscoverySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for DiscoverySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DiscoverySource::OnChainValidatorSet => "OnChainValidatorSet",
                DiscoverySource::File => "File",
                DiscoverySource::Config => "Config",
                DiscoverySource::Rest => "Rest",
            }
        )
    }
}

/// Requests received by the [`ConnectivityManager`] manager actor from upstream modules.
#[derive(Debug, Serialize)]
pub enum ConnectivityRequest {
    /// Update set of discovered peers and associated info
    UpdateDiscoveredPeers(DiscoverySource, PeerSet),
    /// Gets current size of connected peers. This is useful in tests.
    #[serde(skip)]
    GetConnectedSize(oneshot::Sender<usize>),
    /// Gets current size of dial queue. This is useful in tests.
    #[serde(skip)]
    GetDialQueueSize(oneshot::Sender<usize>),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
struct DiscoveredPeerSet {
    peer_set: HashMap<PeerId, DiscoveredPeer>,
}

impl DiscoveredPeerSet {
    #[cfg(test)]
    /// Creates a new discovered peer set from the
    /// specified peer set. This is used for testing.
    pub fn new_from_peer_set(peer_set: HashMap<PeerId, DiscoveredPeer>) -> Self {
        Self { peer_set }
    }

    /// Gets the eligible peers from the discovered peer set
    fn get_eligible_peers(&self) -> PeerSet {
        self.peer_set
            .iter()
            .filter(|(_, peer)| peer.is_eligible())
            .map(|(peer_id, peer)| (*peer_id, peer.into()))
            .collect()
    }

    /// Removes the specified peer from the set if the state is empty
    fn remove_peer_if_empty(&mut self, peer_id: &PeerId) {
        if let Entry::Occupied(entry) = self.peer_set.entry(*peer_id) {
            if entry.get().is_empty() {
                entry.remove();
            }
        }
    }

    /// Updates the last dial time for the specified peer (if one was found)
    fn update_last_dial_time(&mut self, peer_id: &PeerId) {
        if let Some(discovered_peer) = self.peer_set.get_mut(peer_id) {
            discovered_peer.update_last_dial_time()
        }
    }

    /// Returns the ping latency for the specified peer (if one was found)
    fn get_ping_latency_secs(&self, peer_id: &PeerId) -> Option<f64> {
        if let Some(discovered_peer) = self.peer_set.get(peer_id) {
            discovered_peer.ping_latency_secs
        } else {
            None
        }
    }

    /// Updates the ping latency for the specified peer (if one was found)
    fn update_ping_latency_secs(&mut self, peer_id: &PeerId, latency_secs: f64) {
        if let Some(discovered_peer) = self.peer_set.get_mut(peer_id) {
            discovered_peer.set_ping_latency_secs(latency_secs)
        }
    }
}

/// Represents all the information for a discovered peer
#[derive(Clone, Debug, PartialEq, Serialize)]
struct DiscoveredPeer {
    role: PeerRole,
    addrs: Addresses,
    keys: PublicKeys,
    /// The last time the node was dialed
    last_dial_time: SystemTime,
    /// The calculated peer ping latency (secs)
    ping_latency_secs: Option<f64>,
}

impl DiscoveredPeer {
    pub fn new(role: PeerRole) -> Self {
        Self {
            role,
            addrs: Addresses::default(),
            keys: PublicKeys::default(),
            last_dial_time: SystemTime::UNIX_EPOCH,
            ping_latency_secs: None,
        }
    }

    /// Peers without keys are not able to be mutually authenticated to
    pub fn is_eligible(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Peers without addresses can't be dialed to
    pub fn is_eligible_to_be_dialed(&self) -> bool {
        self.is_eligible() && !self.addrs.is_empty()
    }

    /// Returns true iff the peer's addresses and keys are empty
    pub fn is_empty(&self) -> bool {
        self.addrs.is_empty() && self.keys.is_empty()
    }

    /// Updates the last time we tried to connect to this node
    pub fn update_last_dial_time(&mut self) {
        self.last_dial_time = SystemTime::now();
    }

    /// Updates the ping latency for this peer
    pub fn set_ping_latency_secs(&mut self, latency_secs: f64) {
        self.ping_latency_secs = Some(latency_secs);
    }

    /// Based on input, backoff on amount of time to dial a peer again
    pub fn has_dialed_recently(&self) -> bool {
        if let Ok(duration_since_last_dial) = self.last_dial_time.elapsed() {
            duration_since_last_dial < TRY_DIAL_BACKOFF_TIME
        } else {
            false
        }
    }
}

impl PartialOrd for DiscoveredPeer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_dialed_recently = self.has_dialed_recently();
        let other_dialed_recently = other.has_dialed_recently();

        // Less recently dialed is prioritized over recently dialed
        if !self_dialed_recently && other_dialed_recently {
            Some(Ordering::Less)
        } else if self_dialed_recently && !other_dialed_recently {
            Some(Ordering::Greater)
        } else {
            self.role.partial_cmp(&other.role)
        }
    }
}

impl From<&DiscoveredPeer> for Peer {
    fn from(peer: &DiscoveredPeer) -> Self {
        Peer::new(peer.addrs.union(), peer.keys.union(), peer.role)
    }
}

/// A set of `NetworkAddress`'s for a single peer, bucketed by DiscoverySource in
/// priority order.
#[derive(Clone, Default, PartialEq, Serialize)]
struct Addresses([Vec<NetworkAddress>; DiscoverySource::NUM_VARIANTS]);

/// Sets of `x25519::PublicKey`s for a single peer, bucketed by DiscoverySource
/// in priority order.
#[derive(Clone, Default, PartialEq, Serialize)]
struct PublicKeys([HashSet<x25519::PublicKey>; DiscoverySource::NUM_VARIANTS]);

#[derive(Debug)]
enum DialResult {
    Success,
    Cancelled,
    Failed(PeerManagerError),
}

/// The state needed to compute the next dial delay and dial addr for a given
/// peer.
#[derive(Debug, Clone)]
struct DialState<TBackoff> {
    /// The current state of this peer's backoff delay.
    backoff: TBackoff,
    /// The index of the next address to dial. Index of an address in the `DiscoveredPeer`'s
    /// `addrs` entry.
    addr_idx: usize,
}

/// oinori: Generates a fresh random x25519 keypair and derives a PeerId from it.
///
/// Each outbound connection uses a unique ephemeral identity so that validators
/// cannot correlate multiple connections back to the same MEV bot node.
/// The private key is used for the Noise handshake, and the PeerId (derived from
/// the public key) is presented as our identity during the connection prologue.
fn oinori_new_self_peer_id() -> (x25519::PrivateKey, PeerId) {
    let mut rng = StdRng::from_seed(OsRng.gen());
    let key = x25519::PrivateKey::generate(&mut rng);
    (key.clone(), from_identity_public_key(key.public_key()))
}

/////////////////////////
// ConnectivityManager //
/////////////////////////

impl<TBackoff> ConnectivityManager<TBackoff>
where
    TBackoff: Iterator<Item = Duration> + Clone,
{
    /// Creates a new instance of the [`ConnectivityManager`] actor.
    pub fn new(
        network_context: NetworkContext,
        time_service: TimeService,
        peers_and_metadata: Arc<PeersAndMetadata>,
        seeds: PeerSet,
        connection_reqs_tx: ConnectionRequestSender,
        connection_notifs_rx: conn_notifs_channel::Receiver,
        requests_rx: aptos_channels::Receiver<ConnectivityRequest>,
        connectivity_check_interval: Duration,
        backoff_strategy: TBackoff,
        max_delay: Duration,
        outbound_connection_limit: Option<usize>,
        mutual_authentication: bool,
        enable_latency_aware_dialing: bool,
    ) -> Self {
        // Verify that the trusted peers set exists and that it is empty
        let trusted_peers = peers_and_metadata
            .get_trusted_peers(&network_context.network_id())
            .unwrap_or_else(|error| {
                panic!("Trusted peers must exist, but found error: {:?}", error)
            });
        assert!(
            trusted_peers.is_empty(),
            "Trusted peers must be empty. Found: {:?}",
            trusted_peers
        );

        info!(
            NetworkSchema::new(&network_context),
            "{} Initialized connectivity manager", network_context
        );

        let mut connmgr = Self {
            network_context,
            time_service,
            peers_and_metadata,
            connected: HashMap::new(),
            discovered_peers: Arc::new(RwLock::new(DiscoveredPeerSet::default())),
            connection_reqs_tx,
            connection_notifs_rx,
            requests_rx,
            dial_queue: HashMap::new(),
            dial_states: HashMap::new(),
            connectivity_check_interval,
            backoff_strategy,
            max_delay,
            event_id: 0,
            outbound_connection_limit,
            mutual_authentication,
            enable_latency_aware_dialing,
            oinori_peer_connected_log: Arc::new(RwLock::new(HashMap::new())),
        };

        // Set the initial seed config addresses and public keys
        connmgr.handle_update_discovered_peers(DiscoverySource::Config, seeds);
        connmgr
    }

    /// Starts the [`ConnectivityManager`] actor.
    pub async fn start(mut self) {
        // The ConnectivityManager actor is interested in 3 kinds of events:
        // 1. Ticks to trigger connecitvity check. These are implemented using a clock based
        //    trigger in production.
        // 2. Incoming requests to connect or disconnect with a peer.
        // 3. Notifications from PeerManager when we establish a new connection or lose an existing
        //    connection with a peer.
        let mut pending_dials = FuturesUnordered::new();

        let ticker = self.time_service.interval(self.connectivity_check_interval);
        tokio::pin!(ticker);

        // oinori: Check every 3 seconds for connections that exceeded the TTL
        // and need to be rotated with a fresh identity.
        let oinori_reconnect_ticker = self.time_service.interval(Duration::from_secs(3));
        tokio::pin!(oinori_reconnect_ticker);

        info!(
            NetworkSchema::new(&self.network_context),
            "{} Starting ConnectivityManager actor", self.network_context
        );

        loop {
            self.event_id = self.event_id.wrapping_add(1);
            futures::select! {
                _ = ticker.select_next_some() => {
                    self.check_connectivity(&mut pending_dials).await;
                },


                // oinori: rotate expired connections with fresh identities
                _ = oinori_reconnect_ticker.select_next_some() => {
                    self.oinori_reconnect_peers().await;
                },

                req = self.requests_rx.select_next_some() => {
                    self.handle_request(req);
                },
                maybe_notif = self.connection_notifs_rx.next() => {
                    // Shutdown the connectivity manager when the PeerManager
                    // shuts down.
                    match maybe_notif {
                        Some(notif) => {
                            self.handle_control_notification(notif.clone());
                        },
                        None => break,
                    }
                },
                peer_id = pending_dials.select_next_some() => {
                    trace!(
                        NetworkSchema::new(&self.network_context)
                            .remote_peer(&peer_id),
                        "{} Dial complete to {}",
                        self.network_context,
                        peer_id.short_str(),
                    );
                    self.dial_queue.remove(&peer_id);
                },
            }
        }

        warn!(
            NetworkSchema::new(&self.network_context),
            "{} ConnectivityManager actor terminated", self.network_context
        );
    }

    /// oinori: Rotates expired peer connections with fresh random identities.
    ///
    /// This is the core of the connection pool identity rotation mechanism.
    /// Connections older than `expire_duration` (10s) are disconnected and
    /// re-established with a newly generated x25519 keypair, making the validator
    /// perceive each reconnection as a distinct client. At most `update_peer_num_at_once`
    /// (10) connections are rotated per tick to avoid disrupting all connections at once.
    async fn oinori_reconnect_peers(&mut self) {
        let connection_reqs_tx = self.connection_reqs_tx.clone();
        let oinori_peer_connected_log = self.oinori_peer_connected_log.clone();

        // Connections older than this are eligible for identity rotation
        let expire_duration = Duration::from_secs(10);
        // Cap the number of connections rotated per tick to maintain connectivity stability
        let update_peer_num_at_once = 10;

        let reconnect_target_peers: Vec<PeerId> = {
            let mut expired_peers: Vec<PeerId> = vec![];
            for (peer_id, connected_at) in oinori_peer_connected_log.read().iter() {
                if connected_at.elapsed().unwrap() > expire_duration {
                    expired_peers.push(peer_id.clone());
                }
            }

            // Randomly select a subset to avoid rotating the same peers each tick
            expired_peers
                .choose_multiple(&mut rand::thread_rng(), update_peer_num_at_once)
                .cloned()
                .collect::<Vec<PeerId>>()
        };

        // Disconnect and re-dial sequentially to avoid overwhelming the transport layer
        for each_peer_id in reconnect_target_peers.into_iter() {
            match connection_reqs_tx.disconnect_peer(each_peer_id).await {
                Ok(_) => {
                    let eligible_peers = self.discovered_peers.read().peer_set.clone();

                    if let Some(discovered_peer) = eligible_peers.get(&each_peer_id) {
                        let init_dial_state = DialState::new(self.backoff_strategy.clone());
                        let dial_state = self
                            .dial_states
                            .entry(each_peer_id)
                            .or_insert_with(|| init_dial_state);

                        let peer_addr = dial_state.next_addr(&discovered_peer.addrs).clone();
                        let peer_addr = match peer_addr {
                            Some(peer_addr) => peer_addr,
                            None => continue,
                        };

                        // Re-dial with a fresh random identity
                        let (self_peer_private_key, self_peer_id) = oinori_new_self_peer_id();
                        match connection_reqs_tx
                            .dial_peer(
                                self_peer_private_key,
                                self_peer_id,
                                each_peer_id,
                                peer_addr.clone(),
                            )
                            .await
                        {
                            Ok(_) => {
                                oinori_peer_connected_log
                                    .write()
                                    .insert(each_peer_id.clone(), SystemTime::now());
                                self.discovered_peers
                                    .write()
                                    .update_last_dial_time(&each_peer_id)
                            },
                            Err(e) => {
                                error!("oinori reconnect peer: re-dial error {}", e);
                            },
                        }
                    }
                },
                Err(e) => {
                    error!("oinori reconnect peer: disconnect error {}", e);
                },
            }
        }
    }

    /// Returns the trusted peers for the current network context.
    /// If no set exists, an error is logged and None is returned.
    fn get_trusted_peers(&self) -> Option<PeerSet> {
        let network_id = self.network_context.network_id();
        match self.peers_and_metadata.get_trusted_peers(&network_id) {
            Ok(trusted_peers) => Some(trusted_peers),
            Err(error) => {
                error!(
                    NetworkSchema::new(&self.network_context),
                    "Failed to find trusted peers for network context: {:?}, error: {:?}",
                    self.network_context,
                    error
                );
                None
            },
        }
    }

    /// Disconnect from all peers that are no longer eligible.
    ///
    /// For instance, a validator might leave the validator set after a
    /// reconfiguration. If we are currently connected to this validator, calling
    /// this function will close our connection to it.
    async fn close_stale_connections(&mut self) {
        if let Some(trusted_peers) = self.get_trusted_peers() {
            // Identify stale peer connections
            let stale_peers = self
                .connected
                .iter()
                .filter(|(peer_id, _)| !trusted_peers.contains_key(peer_id))
                .filter_map(|(peer_id, metadata)| {
                    // If we're using server only auth, we need to not evict unknown peers
                    // TODO: We should prevent `Unknown` from discovery sources
                    if !self.mutual_authentication
                        && metadata.origin == ConnectionOrigin::Inbound
                        && (metadata.role == PeerRole::ValidatorFullNode
                            || metadata.role == PeerRole::Unknown)
                    {
                        None
                    } else {
                        Some(*peer_id) // The peer is stale
                    }
                });

            // Close existing connections to stale peers
            for stale_peer in stale_peers {
                info!(
                    NetworkSchema::new(&self.network_context).remote_peer(&stale_peer),
                    "{} Closing stale connection to peer {}",
                    self.network_context,
                    stale_peer.short_str()
                );

                if let Err(disconnect_error) =
                    self.connection_reqs_tx.disconnect_peer(stale_peer).await
                {
                    info!(
                        NetworkSchema::new(&self.network_context)
                            .remote_peer(&stale_peer),
                        error = %disconnect_error,
                        "{} Failed to close stale connection to peer {}, error: {}",
                        self.network_context,
                        stale_peer.short_str(),
                        disconnect_error
                    );
                }
            }
        }
    }

    /// Cancel all pending dials to peers that are no longer eligible.
    ///
    /// For instance, a validator might leave the validator set after a
    /// reconfiguration. If there is a pending dial to this validator, calling
    /// this function will remove it from the dial queue.
    async fn cancel_stale_dials(&mut self) {
        if let Some(trusted_peers) = self.get_trusted_peers() {
            // Identify stale peer dials
            let stale_peer_dials: Vec<AccountAddress> = self
                .dial_queue
                .keys()
                .filter(|peer_id| !trusted_peers.contains_key(peer_id))
                .cloned()
                .collect();

            // Remove the stale dials from the dial queue
            for stale_peer_dial in stale_peer_dials {
                debug!(
                    NetworkSchema::new(&self.network_context).remote_peer(&stale_peer_dial),
                    "{} Cancelling stale dial {}",
                    self.network_context,
                    stale_peer_dial.short_str()
                );
                self.dial_queue.remove(&stale_peer_dial);
            }
        }
    }

    /// Identifies a set of peers to dial and queues them for dialing
    async fn dial_eligible_peers<'a>(
        &'a mut self,
        pending_dials: &'a mut FuturesUnordered<BoxFuture<'static, PeerId>>,
    ) {
        for (peer_id, peer) in self.choose_peers_to_dial().await {
            self.queue_dial_peer(peer_id, peer, pending_dials);
        }
    }

    /// Selects a set of peers to dial
    async fn choose_peers_to_dial(&mut self) -> Vec<(PeerId, DiscoveredPeer)> {
        // Get the eligible peers to dial
        let network_id = self.network_context.network_id();
        let role = self.network_context.role();
        let roles_to_dial = network_id.upstream_roles(&role);
        let discovered_peers = self.discovered_peers.read().peer_set.clone();

        let mut eligible_peers: Vec<_> = discovered_peers
            .into_iter()
            .filter(|(peer_id, peer)| {
                peer.is_eligible_to_be_dialed() // The node is eligible to dial
                    && !self.connected.contains_key(peer_id) // The node is not already connected
                    && !self.dial_queue.contains_key(peer_id) // There is no pending dial to this node
                    && roles_to_dial.contains(&peer.role) // We can dial this role
            })
            .collect();

        // oinori: Filter to only geographically close validators (Korea/Japan/Singapore or EU)
        // to minimize network latency for time-sensitive MEV transactions.
        oinori_filter_sort_eligible_peers(&mut eligible_peers);

        // Initialize the dial state for any new peers
        for (peer_id, _) in &eligible_peers {
            self.dial_states
                .entry(*peer_id)
                .or_insert_with(|| DialState::new(self.backoff_strategy.clone()));
        }

        // oinori: Connect to ALL eligible (geographically filtered) peers rather than
        // using the original connection limit logic. Maintaining connections to many
        // validators increases the chance of fast transaction submission for MEV.
        eligible_peers
    }

    /// Pings the eligible peers to calculate their ping latencies
    /// and updates the discovered peer state accordingly.
    async fn ping_eligible_peers(&mut self, eligible_peers: Vec<(PeerId, DiscoveredPeer)>) {
        // Identify the eligible peers that don't already have latency information
        let peers_to_ping = eligible_peers
            .into_iter()
            .filter(|(_, peer)| peer.ping_latency_secs.is_none())
            .collect::<Vec<_>>();

        // If there are no peers to ping, return early
        let num_peers_to_ping = peers_to_ping.len();
        if num_peers_to_ping == 0 {
            return;
        }

        // Spawn a task that pings each peer concurrently
        let ping_start_time = Instant::now();
        let mut ping_tasks = vec![];
        for (peer_id, peer) in peers_to_ping.into_iter() {
            // Get the network address for the peer
            let network_context = self.network_context;
            let network_address = match self.dial_states.get(&peer_id) {
                Some(dial_state) => match dial_state.random_addr(&peer.addrs) {
                    Some(network_address) => network_address.clone(),
                    None => {
                        warn!(
                            NetworkSchema::new(&network_context),
                            "Peer {} does not have a network address!",
                            peer_id.short_str()
                        );
                        continue; // Continue onto the next peer
                    },
                },
                None => {
                    warn!(
                        NetworkSchema::new(&network_context),
                        "Peer {} does not have a dial state!",
                        peer_id.short_str()
                    );
                    continue; // Continue onto the next peer
                },
            };

            // Ping the peer
            let ping_task = spawn_latency_ping_task(
                network_context,
                peer_id,
                network_address,
                self.discovered_peers.clone(),
            );

            // Add the task to the list of ping tasks
            ping_tasks.push(ping_task);
        }

        // Wait for all the ping tasks to complete (or timeout)
        let num_ping_tasks = ping_tasks.len();
        join_all(ping_tasks).await;

        // Log the peer ping latencies
        log_peer_ping_latencies(
            self.network_context,
            self.discovered_peers.clone(),
            num_peers_to_ping,
            num_ping_tasks,
            ping_start_time,
        );
    }

    /// Queues a dial to the specified peer
    fn queue_dial_peer<'a>(
        &'a mut self,
        peer_id: PeerId,
        peer: DiscoveredPeer,
        pending_dials: &'a mut FuturesUnordered<BoxFuture<'static, PeerId>>,
    ) {
        // If we're attempting to dial a Peer we must not be connected to it. This ensures that
        // newly eligible, but not connected to peers, have their counter initialized properly.
        counters::peer_connected(&self.network_context, &peer_id, 0);

        // Get the peer's dial state
        let dial_state = match self.dial_states.get_mut(&peer_id) {
            Some(dial_state) => dial_state,
            None => {
                // The peer should have a dial state! If not, log an error and return.
                error!(
                    NetworkSchema::new(&self.network_context).remote_peer(&peer_id),
                    "{} Peer {} does not have a dial state!",
                    self.network_context,
                    peer_id.short_str()
                );
                return;
            },
        };

        // Choose the next addr to dial for this peer. Currently, we just
        // round-robin the selection, i.e., try the sequence:
        // addr[0], .., addr[len-1], addr[0], ..
        let addr = match dial_state.next_addr(&peer.addrs) {
            Some(addr) => addr.clone(),
            None => {
                warn!(
                    NetworkSchema::new(&self.network_context).remote_peer(&peer_id),
                    "{} Peer {} does not have any network addresses!",
                    self.network_context,
                    peer_id.short_str()
                );
                return;
            },
        };

        // Using the DialState's backoff strategy, compute the delay until
        // the next dial attempt for this peer.
        let dial_delay = dial_state.next_backoff_delay(self.max_delay);
        let f_delay = self.time_service.sleep(dial_delay);

        let (cancel_tx, cancel_rx) = oneshot::channel();

        let network_context = self.network_context;
        let oinori_peer_connected_log = self.oinori_peer_connected_log.clone();
        // Create future which completes by either dialing after calculated
        // delay or on cancellation.
        let connection_reqs_tx = self.connection_reqs_tx.clone();
        let f = async move {
            // We dial after a delay. The dial can be canceled by sending to or dropping
            // `cancel_rx`.
            let dial_result = futures::select! {
                _ = f_delay.fuse() => {
                    info!(
                        NetworkSchema::new(&network_context)
                            .remote_peer(&peer_id)
                            .network_address(&addr),
                        "{} Dialing peer {} at {}",
                        network_context,
                        peer_id.short_str(),
                        addr
                    );


                    // oinori: Generate a fresh random identity for this connection.
                    // The validator will see a unique PeerId for each dial attempt,
                    // preventing identity-based rate limiting or banning.
                    let (self_peer_private_key, self_peer_id) = oinori_new_self_peer_id();
                    match connection_reqs_tx.dial_peer(
                        self_peer_private_key, self_peer_id,
                        peer_id.clone(), addr.clone()).await {
                        Ok(_) => {
                            // Record connection time for the TTL-based rotation logic
                            let mut oinori_peer_connected_log_writer = oinori_peer_connected_log.write();
                            oinori_peer_connected_log_writer.insert(peer_id, SystemTime::now());
                            DialResult::Success
                        },
                        Err(e) => DialResult::Failed(e),
                    }
                },
                _ = cancel_rx.fuse() => DialResult::Cancelled,
            };
            log_dial_result(network_context, peer_id, addr, dial_result);
            // Send peer_id as future result so it can be removed from dial queue.
            peer_id
        };
        pending_dials.push(f.boxed());

        // Update last dial time
        self.discovered_peers
            .write()
            .update_last_dial_time(&peer_id);
        self.dial_queue.insert(peer_id, cancel_tx);
    }

    // Note: We do not check that the connections to older incarnations of a node are broken, and
    // instead rely on the node moving to a new epoch to break connections made from older
    // incarnations.
    async fn check_connectivity<'a>(
        &'a mut self,
        pending_dials: &'a mut FuturesUnordered<BoxFuture<'static, PeerId>>,
    ) {
        trace!(
            NetworkSchema::new(&self.network_context),
            "{} Checking connectivity",
            self.network_context
        );

        // Log the eligible peers with addresses from discovery
        sample!(SampleRate::Duration(Duration::from_secs(60)), {
            info!(
                NetworkSchema::new(&self.network_context),
                discovered_peers = ?self.discovered_peers,
                "Active discovered peers"
            )
        });

        // Cancel dials to peers that are no longer eligible.
        self.cancel_stale_dials().await;
        // Disconnect from connected peers that are no longer eligible.
        self.close_stale_connections().await;
        // Dial peers which are eligible but are neither connected nor queued for dialing in the
        // future.
        self.dial_eligible_peers(pending_dials).await;

        // Update the metrics for any peer ping latencies
        self.update_ping_latency_metrics();
    }

    /// Updates the metrics for tracking pre-dial and connected peer ping latencies
    fn update_ping_latency_metrics(&self) {
        // Update the pre-dial peer ping latencies
        for (_, peer) in self.discovered_peers.read().peer_set.iter() {
            if let Some(ping_latency_secs) = peer.ping_latency_secs {
                counters::observe_pre_dial_ping_time(&self.network_context, ping_latency_secs);
            }
        }

        // Update the connected peer ping latencies
        for peer_id in self.connected.keys() {
            if let Some(ping_latency_secs) =
                self.discovered_peers.read().get_ping_latency_secs(peer_id)
            {
                counters::observe_connected_ping_time(&self.network_context, ping_latency_secs);
            }
        }
    }

    fn handle_request(&mut self, req: ConnectivityRequest) {
        trace!(
            NetworkSchema::new(&self.network_context),
            connectivity_request = req,
            "{} Handling ConnectivityRequest",
            self.network_context
        );

        match req {
            ConnectivityRequest::UpdateDiscoveredPeers(src, discovered_peers) => {
                trace!(
                    NetworkSchema::new(&self.network_context),
                    "{} Received updated list of discovered peers: src: {:?}",
                    self.network_context,
                    src,
                );

                self.handle_update_discovered_peers(src, discovered_peers);
            },
            ConnectivityRequest::GetDialQueueSize(sender) => {
                sender.send(self.dial_queue.len()).unwrap();
            },
            ConnectivityRequest::GetConnectedSize(sender) => {
                sender.send(self.connected.len()).unwrap();
            },
        }
    }

    /// Handles an update for newly discovered peers. This typically
    /// occurs at node startup, and on epoch changes.
    fn handle_update_discovered_peers(
        &mut self,
        src: DiscoverySource,
        new_discovered_peers: PeerSet,
    ) {
        // Log the update event
        info!(
            NetworkSchema::new(&self.network_context),
            "{} Received updated list of discovered peers! Source: {:?}, num peers: {:?}",
            self.network_context,
            src,
            new_discovered_peers.len()
        );

        // Remove peers that no longer have relevant network information
        let mut keys_updated = false;
        let mut peers_to_check_remove = Vec::new();
        for (peer_id, peer) in self.discovered_peers.write().peer_set.iter_mut() {
            let new_peer = new_discovered_peers.get(peer_id);
            let check_remove = if let Some(new_peer) = new_peer {
                if new_peer.keys.is_empty() {
                    keys_updated |= peer.keys.clear_src(src);
                }
                if new_peer.addresses.is_empty() {
                    peer.addrs.clear_src(src);
                }
                new_peer.addresses.is_empty() && new_peer.keys.is_empty()
            } else {
                keys_updated |= peer.keys.clear_src(src);
                peer.addrs.clear_src(src);
                true
            };
            if check_remove {
                peers_to_check_remove.push(*peer_id);
            }
        }

        // Remove peers that no longer have state
        for peer_id in peers_to_check_remove {
            self.discovered_peers.write().remove_peer_if_empty(&peer_id);
        }

        // Make updates to the peers accordingly
        for (peer_id, discovered_peer) in new_discovered_peers {
            // Don't include ourselves, because we don't need to dial ourselves
            if peer_id == self.network_context.peer_id() {
                continue;
            }

            // Create the new `DiscoveredPeer`, role is set when a `Peer` is first discovered
            let mut discovered_peers = self.discovered_peers.write();
            let peer = discovered_peers
                .peer_set
                .entry(peer_id)
                .or_insert_with(|| DiscoveredPeer::new(discovered_peer.role));

            // Update the peer's pubkeys
            let mut peer_updated = false;
            if peer.keys.update(src, discovered_peer.keys) {
                info!(
                    NetworkSchema::new(&self.network_context)
                        .remote_peer(&peer_id)
                        .discovery_source(&src),
                    "{} pubkey sets updated for peer: {}, pubkeys: {}",
                    self.network_context,
                    peer_id.short_str(),
                    peer.keys
                );
                keys_updated = true;
                peer_updated = true;
            }

            // Update the peer's addresses
            if peer.addrs.update(src, discovered_peer.addresses) {
                info!(
                    NetworkSchema::new(&self.network_context).remote_peer(&peer_id),
                    network_addresses = &peer.addrs,
                    "{} addresses updated for peer: {}, update src: {:?}, addrs: {}",
                    self.network_context,
                    peer_id.short_str(),
                    src,
                    &peer.addrs,
                );
                peer_updated = true;
            }

            // If we're currently trying to dial this peer, we reset their
            // dial state. As a result, we will begin our next dial attempt
            // from the first address (which might have changed) and from a
            // fresh backoff (since the current backoff delay might be maxed
            // out if we can't reach any of their previous addresses).
            if peer_updated {
                if let Some(dial_state) = self.dial_states.get_mut(&peer_id) {
                    *dial_state = DialState::new(self.backoff_strategy.clone());
                }
            }
        }

        // update eligible peers accordingly
        if keys_updated {
            // For each peer, union all of the pubkeys from each discovery source
            // to generate the new eligible peers set.
            let new_eligible = self.discovered_peers.read().get_eligible_peers();

            // Swap in the new eligible peers set
            if let Err(error) = self
                .peers_and_metadata
                .set_trusted_peers(&self.network_context.network_id(), new_eligible)
            {
                error!(
                    NetworkSchema::new(&self.network_context),
                    error = %error,
                    "Failed to update trusted peers set"
                );
            }
        }
    }

    fn handle_control_notification(&mut self, notif: peer_manager::ConnectionNotification) {
        trace!(
            NetworkSchema::new(&self.network_context),
            connection_notification = notif,
            "Connection notification"
        );
        match notif {
            peer_manager::ConnectionNotification::NewPeer(metadata, _network_id) => {
                let peer_id = metadata.remote_peer_id;
                counters::peer_connected(&self.network_context, &peer_id, 1);
                self.connected.insert(peer_id, metadata);

                // Cancel possible queued dial to this peer.
                self.dial_states.remove(&peer_id);
                self.dial_queue.remove(&peer_id);
            },
            peer_manager::ConnectionNotification::LostPeer(metadata, _network_id) => {
                let peer_id = metadata.remote_peer_id;
                if let Some(stored_metadata) = self.connected.get(&peer_id) {
                    // Remove node from connected peers list.

                    counters::peer_connected(&self.network_context, &peer_id, 0);

                    info!(
                        NetworkSchema::new(&self.network_context)
                            .remote_peer(&peer_id)
                            .connection_metadata(&metadata),
                        stored_metadata = stored_metadata,
                        "{} Removing peer '{}' metadata: {}, vs event metadata: {}",
                        self.network_context,
                        peer_id.short_str(),
                        stored_metadata,
                        metadata
                    );
                    self.connected.remove(&peer_id);
                } else {
                    info!(
                        NetworkSchema::new(&self.network_context)
                            .remote_peer(&peer_id)
                            .connection_metadata(&metadata),
                        "{} Ignoring stale lost peer event for peer: {}, addr: {}",
                        self.network_context,
                        peer_id.short_str(),
                        metadata.addr
                    );
                }
            },
        }
    }

    #[cfg(test)]
    /// Returns the set of connected peers (for test purposes)
    fn get_connected_peers(&self) -> HashMap<PeerId, ConnectionMetadata> {
        self.connected.clone()
    }
}

fn log_dial_result(
    network_context: NetworkContext,
    peer_id: PeerId,
    addr: NetworkAddress,
    dial_result: DialResult,
) {
    match dial_result {
        DialResult::Success => {
            info!(
                NetworkSchema::new(&network_context)
                    .remote_peer(&peer_id)
                    .network_address(&addr),
                "{} Successfully connected to peer: {} at address: {}",
                network_context,
                peer_id.short_str(),
                addr
            );
        },
        DialResult::Cancelled => {
            info!(
                NetworkSchema::new(&network_context).remote_peer(&peer_id),
                "{} Cancelled pending dial to peer: {}",
                network_context,
                peer_id.short_str()
            );
        },
        DialResult::Failed(err) => match err {
            PeerManagerError::AlreadyConnected(a) => {
                info!(
                    NetworkSchema::new(&network_context)
                        .remote_peer(&peer_id)
                        .network_address(&a),
                    "{} Already connected to peer: {} at address: {}",
                    network_context,
                    peer_id.short_str(),
                    a
                );
            },
            e => {
                info!(
                    NetworkSchema::new(&network_context)
                        .remote_peer(&peer_id)
                        .network_address(&addr),
                    error = %e,
                    "{} Failed to connect to peer: {} at address: {}; error: {}",
                    network_context,
                    peer_id.short_str(),
                    addr,
                    e
                );
            },
        },
    }
}

/// Logs the total and individual ping latencies
fn log_peer_ping_latencies(
    network_context: NetworkContext,
    discovered_peers: Arc<RwLock<DiscoveredPeerSet>>,
    total_peers_to_ping: usize,
    num_peers_pinged: usize,
    ping_start_time: Instant,
) {
    // Log the total ping latency time
    let ping_latency_duration = Instant::now().duration_since(ping_start_time);
    info!(
        NetworkSchema::new(&network_context),
        "Finished pinging eligible peers! Total peers to ping: {}, num peers pinged: {}, time: {} secs",
        total_peers_to_ping,
        num_peers_pinged,
        ping_latency_duration.as_secs_f64()
    );

    // Log the ping latencies for the eligible peers (sorted by latency)
    let eligible_peers = discovered_peers.read().peer_set.clone();
    let eligible_peers_and_latencies = eligible_peers
        .into_iter()
        .map(|(peer_id, peer)| (peer_id, peer.ping_latency_secs))
        .collect::<Vec<_>>();
    let sorted_eligible_peers_and_latencies = eligible_peers_and_latencies
        .iter()
        .sorted_by_key(|(_, ping_latency_secs)| ping_latency_secs.map(OrderedFloat))
        .collect::<Vec<_>>();
    info!(
        NetworkSchema::new(&network_context),
        "Sorted eligible peers with recorded ping latencies: {:?}",
        sorted_eligible_peers_and_latencies
    );
}

/// Spawns a task that pings the peer at the specified
/// network address and updates the peer's ping latency.
fn spawn_latency_ping_task(
    network_context: NetworkContext,
    peer_id: AccountAddress,
    network_address: NetworkAddress,
    discovered_peers: Arc<RwLock<DiscoveredPeerSet>>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        // Extract the socket addresses from the network address
        let socket_addresses = match network_address.to_socket_addrs() {
            Ok(socket_addresses) => socket_addresses.collect::<Vec<_>>(),
            Err(error) => {
                warn!(
                    NetworkSchema::new(&network_context),
                    "Failed to resolve network address {:?}: {}", network_address, error
                );
                return;
            },
        };

        // If no socket addresses were found, log an error and return
        if socket_addresses.is_empty() {
            warn!(
                NetworkSchema::new(&network_context),
                "Peer {} does not have any socket addresses for network address {:?}!",
                peer_id.short_str(),
                network_address,
            );
            return;
        }

        // Limit the number of socket addresses we'll try to connect to
        let socket_addresses = socket_addresses
            .iter()
            .take(MAX_SOCKET_ADDRESSES_TO_PING)
            .collect::<Vec<_>>();

        // Attempt to connect to the socket addresses over TCP and time the connection
        for socket_address in socket_addresses {
            // Start the ping timer
            let start_time = Instant::now();

            // Attempt to connect to the socket address
            if let Ok(tcp_stream) = TcpStream::connect_timeout(
                socket_address,
                Duration::from_secs(MAX_CONNECTION_TIMEOUT_SECS),
            ) {
                // We connected successfully, update the peer's ping latency
                let ping_latency_secs = start_time.elapsed().as_secs_f64();
                discovered_peers
                    .write()
                    .update_ping_latency_secs(&peer_id, ping_latency_secs);

                // Attempt to terminate the TCP stream cleanly
                if let Err(error) = tcp_stream.shutdown(Shutdown::Both) {
                    warn!(
                        NetworkSchema::new(&network_context),
                        "Failed to terminate TCP stream to peer {} after pinging: {}",
                        peer_id.short_str(),
                        error
                    );
                }

                return;
            } else {
                // Log an error if we failed to connect to the socket address
                info!(
                    NetworkSchema::new(&network_context),
                    "Failed to ping peer {} at socket address {:?} after pinging",
                    peer_id.short_str(),
                    socket_address
                );
            }
        }
    })
}

/////////////////////
// DiscoverySource //
/////////////////////

impl DiscoverySource {
    fn as_usize(self) -> usize {
        self as u8 as usize
    }
}

///////////////
// Addresses //
///////////////

impl Addresses {
    fn len(&self) -> usize {
        self.0.iter().map(Vec::len).sum()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Update the addresses for the `DiscoverySource` bucket. Return `true` if
    /// the addresses have actually changed.
    fn update(&mut self, src: DiscoverySource, addrs: Vec<NetworkAddress>) -> bool {
        let src_idx = src.as_usize();
        if self.0[src_idx] != addrs {
            self.0[src_idx] = addrs;
            true
        } else {
            false
        }
    }

    fn clear_src(&mut self, src: DiscoverySource) -> bool {
        self.update(src, Vec::new())
    }

    fn get(&self, idx: usize) -> Option<&NetworkAddress> {
        self.0.iter().flatten().nth(idx)
    }

    /// The Union isn't stable, and order is completely disregarded
    fn union(&self) -> Vec<NetworkAddress> {
        let set: HashSet<_> = self.0.iter().flatten().cloned().collect();
        set.into_iter().collect()
    }
}

impl fmt::Display for Addresses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write without the typical "Addresses(..)" around the output to reduce
        // debug noise.
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Debug for Addresses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

////////////////
// PublicKeys //
////////////////

impl PublicKeys {
    fn len(&self) -> usize {
        self.0.iter().map(HashSet::len).sum()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn update(&mut self, src: DiscoverySource, pubkeys: HashSet<x25519::PublicKey>) -> bool {
        let src_idx = src.as_usize();
        if self.0[src_idx] != pubkeys {
            self.0[src_idx] = pubkeys;
            true
        } else {
            false
        }
    }

    fn clear_src(&mut self, src: DiscoverySource) -> bool {
        self.update(src, HashSet::new())
    }

    fn union(&self) -> HashSet<x25519::PublicKey> {
        self.0.iter().flatten().copied().collect()
    }
}

impl fmt::Display for PublicKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write without the typical "PublicKeys(..)" around the output to reduce
        // debug noise.
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Debug for PublicKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

///////////////
// DialState //
///////////////

impl<TBackoff> DialState<TBackoff>
where
    TBackoff: Iterator<Item = Duration> + Clone,
{
    fn new(backoff: TBackoff) -> Self {
        Self {
            backoff,
            addr_idx: 0,
        }
    }

    /// Returns the address to dial (specified by the index) for this peer
    fn get_addr_at_index<'a>(
        &self,
        addr_index: usize,
        addrs: &'a Addresses,
    ) -> Option<&'a NetworkAddress> {
        addrs.get(addr_index % addrs.len())
    }

    /// Returns the current address to dial for this peer and updates
    /// the internal state to point to the next address.
    fn next_addr<'a>(&mut self, addrs: &'a Addresses) -> Option<&'a NetworkAddress> {
        let curr_addr = self.get_addr_at_index(self.addr_idx, addrs);
        self.addr_idx = self.addr_idx.wrapping_add(1);
        curr_addr
    }

    /// Returns a random address to dial for this peer
    fn random_addr<'a>(&self, addrs: &'a Addresses) -> Option<&'a NetworkAddress> {
        let addr_index = ::rand_latest::thread_rng().gen_range(0..addrs.len());
        self.get_addr_at_index(addr_index, addrs)
    }

    fn next_backoff_delay(&mut self, max_delay: Duration) -> Duration {
        let jitter = jitter(MAX_CONNECTION_DELAY_JITTER);

        min(max_delay, self.backoff.next().unwrap_or(max_delay)) + jitter
    }
}

/// oinori: Filters peers to only include validators in geographically close regions.
///
/// For MEV, network latency directly impacts profitability. By connecting only to
/// validators co-located in nearby data centers (Korea/Japan/Singapore or Europe),
/// we minimize round-trip time for transaction submission.
///
/// The region is selected via the `KOREAN_VALIDATOR` environment variable:
/// - "1": connect to Korea/Japan/Singapore region validators
/// - otherwise: connect to European region validators
fn oinori_filter_sort_eligible_peers(eligible: &mut Vec<(PeerId, DiscoveredPeer)>) {
    if eligible.len() == 1 {
        // Only the seed peer is present; don't filter it out
        return;
    }

    let validators_filter = if std::env::var("KOREAN_VALIDATOR")
        .unwrap_or_else(|_| "0".to_string())
        .as_str()
        == "1"
    {
        korean_validators()
    } else {
        europe_validators()
    };

    let mut i = 0;
    while i < eligible.len() {
        if let Some((peer_id, _)) = eligible.get(i) {
            if !validators_filter.contains(&peer_id.to_hex_literal()) {
                let _ = eligible.remove(i);
            } else {
                i += 1;
            }
        } else {
            break;
        }
    }
}

/// oinori: Validator PeerIDs located in the Korea/Japan/Singapore region.
fn korean_validators() -> HashSet<String> {
    // Seoul / Tokyo / Yongsan-gu
    [
        "0x4c893cf8d614f503e982df5b48e9443bab4ea6be4f922a5d3858305c0a3af3ee",
        "0x99d623e94981c2a6d41306785d69d66c237c6d3ac305cb6f32307785fde7fda2",
        "0x371dfdee9fbecaa358683936c5946aa28393067fa60e1e185c4250da461e15cf",
        "0xadbfddd3d10cad1eed59d2c0793df0bfd1d16255309c38b34b8351daaddecccc",
        "0x947c6451ad9a8659cba4e60a6da41086eb66e73fc07936bd3b7044873b5e094d",
        "0x4cc22e5d2e84794c3c672b0b34145898ee971a4ff042eabae23627547f6dac80",
        "0xa286a9187c86b4a3cef60478e5c61bb45757b795a8e6577ebbf0fa990db574cc",
        "0xfc307ecd61ac025f5ec1828d2114d11b474b4d8a1b0c228c38614dc96b940048",
        "0xc29a231581f243ddcbf97de8b93c8aabb466dd6912ce5ba12579ff2f13cb8da5",
        "0x46b51b26710797faed701722bfc809386110c9720328f2b431e193cb5dc161f7",
        "0x32ad233a939bfbafb8d9056c0ae2eba58828d8baf5582277578ced38477f0f14",
        "0xae139fd4e266beb28925227dcd220d89348117680e48d4aa20685ae102ae2a13",
        "0xa6eb54c274da96778a6702740a763f6aa7b02eef3d1efc2623d28289d185118",
        "0x6c8a3474cb49202515d121fea0f3217d303e41f6bdc43e615f1cd90855118089",
        // Singapore
        "0xe8fac9c24a3bfc71ed79d466fa3c2c2b4787f1f9686e1d1e9e97fde473898788",
        "0x649127a4d1b26b4cf936f7b795917dbcb571c59c2926ae473fe4d9df3575790e",
    ]
    .into_iter()
    .map(|x| x.to_string())
    .collect::<HashSet<String>>()
}

/// oinori: Validator PeerIDs located in the European region.
fn europe_validators() -> HashSet<String> {
    //┌──────┬────────────────────────────────────────────────────────────────────┬───────────────────────────────────────────────────────────────────────────────┬─────────────┐
    //│ rank ┆ account_address                                                    ┆ ip_adrress_or_host                                                            ┆ country     │
    //│ ---  ┆ ---                                                                ┆ ---                                                                           ┆ ---         │
    //│ i64  ┆ str                                                                ┆ str                                                                           ┆ str         │
    //╞══════╪════════════════════════════════════════════════════════════════════╪═══════════════════════════════════════════════════════════════════════════════╪═════════════╡
    //│ 2    ┆ 0xef191e643c2ed2d103de4109a506e020f5dde818cbc1ffe5e4ee36328cd22e9b ┆ val1.mainnet.aptos.p2p.org                                                    ┆ Netherlands │
    //│ 3    ┆ 0xb4a4f1ef8b0702d85547dc444571a473f736e1205a86db36dad13815ad9bbbf6 ┆ 162.19.83.220                                                                 ┆ France      │
    //│ 7    ┆ 0x86da6288dd50be1653aa2a8269acf881d3b4b414938ca7ab3b662c2dfeb794df ┆ aptos-validator.polkachu.com                                                  ┆ Germany     │
    //│ 9    ┆ 0xb5064f2e6d2d4887c836c8cbd90d23a75cd7afd4530e4a82f06bf528abcb30d6 ┆ 212.95.51.89                                                                  ┆ Germany     │
    //│ 19   ┆ 0x81d41a31fb5d3f827d188511e6fca1ddbed6cfa0ffbd46fa6d84a7adfe59f356 ┆ 80.190.129.50                                                                 ┆ Germany     │
    //│ 22   ┆ 0xc32f662cd9718f02d8a8e5628f8f642fa27cd9b5f457b406ed734901a4939e34 ┆ aptos-validator.stakely.io                                                    ┆ Poland      │
    //│ 25   ┆ 0xc807d24a6ac70599cc9b72c649eb6b00c34a0e9c704447c44d75fade07213a26 ┆ v.aptos.mainnet.republiccrypto-source.info                                    ┆ Netherlands │
    //│ 30   ┆ 0x9a41af5c002f91dc29cb2843ab823e3d6cdaa6cc86e6331646a4f1328609a744 ┆ validator.mainnet.aptos.kiln.fi                                               ┆ France      │
    //│ 31   ┆ 0x3d654d35d558d6381fe6a0484d81c4fed128cd8adc1ca7a1482c297da80ee039 ┆ vn.mainnet.pontem.network                                                     ┆ Germany     │
    //│ 34   ┆ 0x1c6a051d9a5ec4cf4200bab21a794d3afe8259d625f6b78aa4d01ab4652327d5 ┆ aptos-main.everstake.one                                                      ┆ Germany     │
    //│ 35   ┆ 0x9bfd93ebaa1efd65515642942a607eeca53a0188c04c21ced646d2f0b9f551e8 ┆ geuw4c-ccn-aptos-mainnet-val1.jumpisolated.com                                ┆ Netherlands │
    //│ 61   ┆ 0x6099edbe54f242bad50020dfd67646b1e46282999483e7064e70f02f7ea3c15  ┆ 141.95.126.4                                                                  ┆ Germany     │
    //│ 79   ┆ 0xe1abb4ea11673aad3137052dce831eb20a1907815cb4a03ccac38ba7a9e988d3 ┆ aptos-validator.nelrann.org                                                   ┆ Poland      │
    //│ 81   ┆ 0xbcb83e07eed46120a0143bd9861ea82495293228b9cd64cc43bda722edaeb7ad ┆ aptos.mindheartsoul.org                                                       ┆ Germany     │
    //│ 82   ┆ 0x302b69423fde5c7dd333ee50d48df16f076072e61f95e0f6db5685ee6c70d35b ┆ 141.95.99.105                                                                 ┆ Germany     │
    //│ 83   ┆ 0x85bec1dcb618004064ab52a594321e8f047bac53be8244c5d10ac136128e30f7 ┆ aptos-validator0.taras.uno                                                    ┆ France      │
    //│ 84   ┆ 0xd2dc57e95669c9bf102cb5693fe8b2a8b10d330213d7ddd9fb417f86f183dc1d ┆ 144.76.114.5                                                                  ┆ Germany     │
    //│ 85   ┆ 0xe9f7d36334768683b85ec1df892dfd8fc389c8f6dbe0b890195d579a6e390c78 ┆ 85.10.199.23                                                                  ┆ Germany     │
    //│ 86   ┆ 0x71debfa733376c4f2dc8970de8962931fe107c7dc1daad3bf189101bfd4adc4b ┆ aptos.mainnet.validator.justlstn.dev                                          ┆ Poland      │
    //│ 87   ┆ 0x904bc056aa17a45c2715113e06cb42f5762581a7b839206f5348938db69425be ┆ aptos-mainnet-validator.stakemind.io                                          ┆ Germany     │
    //│ 88   ┆ 0xf2b6feeea4546648c257c7e33ff4f0aa8db3bb2949cca065c9db015478363458 ┆ validator.aptos.quantnode.tech                                                ┆ Germany     │
    //
    //│ !8    ┆ 0x48179547b95487f77214c12e2561d53531122a91e5f79ee9be968ce8185bc444 ┆ aptos.lavenderfive.com                                                        ┆ Finland     │
    //│ !14   ┆ 0xffb19e4519bc434d3a064652064d14dd34fabc8f785f6be8670d7465d8419c45 ┆ validator.envoys-aptos.com                                                    ┆ Ireland     │
    //│ !23   ┆ 0x9da88926fd4d773fd499fc41830a82fe9c9ff3508435e7a16b2d8f529e77cdda ┆ add6dfc3ccd12420ab961eccaebeb7ba-460d5d919261f748.elb.eu-west-1.amazonaws.com ┆ Ireland     │
    //│ !26   ┆ 0x209154f6e735562ff13b963b91588039a162fa95f409c068f917f27b762dfff8 ┆ validator-1.mainnet.aptos.fernlabs.xyz                                        ┆ Sweden      │
    //│ !27   ┆ 0xf05d2fe6618a3cb4258e94f2da24c1ee71ae4e7942b4f8b5836ad5267e59568f ┆ bd-aptos-main-validator-01.bdnodes.net                                        ┆ Ireland     │
    //│ !37   ┆ 0x39af34a3586c7ac7c4947885f3693bd748969d70b54db38655419060c4fe833f ┆ validator.ae9475fe-7618-4d5f-ac43-d8c1a2feaba4.aptos.bison.run                ┆ Ireland     │
    //│ !39   ┆ 0xb87476d8877cfc324a520cef4d0cd6e42e26ec3ce613a8127220e2e444561e16 ┆ aeuw1b-ccn-aptos-mainnet-val2.jumpisolated.com                                ┆ Ireland     │
    //│ !42   ┆ 0xf25c7b01c68f7ca3583dde23cbaefd14f930f6655237be627435b9727b7c9034 ┆ validator.baa57d9f-8c15-4166-b325-e86b85e5ddc2.aptos.bison.run                ┆ Ireland     │
    //│ !43   ┆ 0x60b7bb4774ca6bebe6fd9204252f3abcd528afa497854d8fd0e19c1c6d7616f7 ┆ validator.14ab83ed-f362-4d0a-abde-bcf2975eaf7f.aptos.bison.run                ┆ Ireland     │
    //│ !44   ┆ 0x99d3561f2c46a3755e681a5b2199886720257404856cd3eb2b1c2566e585ea11 ┆ validator.17de0f1e-adf0-457e-af3a-a2889f479988.aptos.bison.run                ┆ Ireland     │
    //│ !45   ┆ 0x29a37d38db6717a61ba0cad4ffe01d15a320888601f15b3cf920feb0f9813abf ┆ validator.099a5cfb-3911-474d-8e3c-eac77aba4a01.aptos.bison.run                ┆ Ireland     │
    //
    //
    //│ !38   ┆ 0x22dd12212d76bc5844b4a0605bf59fa56547f42a40fb42267e29adca5efecfc8 ┆ validator-3.mainnet.aptos.fernlabs.xyz                                        ┆ Sweden      │
    //│ !48   ┆ 0xb225277ba076b6620447f7b10abb30911966eb66d25fb526b7f09c2227425a11 ┆ aptos-staked-7.prod-eks-ap-northeast-2.staked.cloud                           ┆ Sweden      │
    //│ !60   ┆ 0xe8a71d40da724ac1ff522d9686dbf1da21bab2d803441df73c86c4bae76d399  ┆ aptos-staked-4.prod-eks-ap-northeast-2.staked.cloud                           ┆ Sweden      │
    //│ !52   ┆ 0x5d31f0d7a171e5ee6a6e2f11917f5a157d745e993f299a8b75acb961bd9da4e0 ┆ validator.ad6f7ef7-feb4-4988-9310-a618dbed79cc.aptos.bison.run                ┆ Ireland     │
    //│ !57   ┆ 0x29c4513282335179295fa7c6a225ca9e2e17b3e14e30a463969d29b87ccb8db6 ┆ validator.f3585fdb-f7f7-4cbc-aa05-d17e15618455.aptos.bison.run                ┆ Ireland     │
    //│ !71   ┆ 0xc48683f9f684d2845c18e8d33d5f6c8581acfc1eb1c78b1de25562d136877e10 ┆ s2zs55mnbe30e46g.node2.cloud-b.mainnet.aptoslabs.com                          ┆ Sweden      │
    //│ !67   ┆ 0x1187093055547879056b22e3d3e80f491bc1b4387c3642d934da0fedaa5e5f28 ┆ validator.ac8829ee-ce3b-4342-b057-8c4cb7ee1b5b.aptos.bison.run                ┆ Ireland     │
    //│ !90   ┆ 0x4cfeaf68aacfd0cd459c292837854ce058b9ba6c1cc35e8684b1a5030b1dc922 ┆ 65.109.70.119                                                                 ┆ Finland     │
    //│ !93   ┆ 0xb840f1cfae5aaa4b50a11a6a23e2e859717e2f355fcd614592b219fc4d4e44a3 ┆ validator.dott3.click                                                         ┆ Ireland     │
    //│ !96   ┆ 0x447fc1fc130cf9245d9ca291785d0f8a00c3ebdc26b2e4c6165801b0af01c917 ┆ 65.109.70.110                                                                 ┆ Finland     │
    //│ !100  ┆ 0x18a3ef480b7883d05b917873a8b6d5f5a5f763372b650242ede89836a67cdce8 ┆ validator.aptoslabs.ltd                                                       ┆ Germany     │
    //│ !101  ┆ 0x8d5c9e82d612c390b5dea347918b6421146bb638724854300eed065cc4211a6e ┆ v.aptoslabs.cc                                                                ┆ Finland     │
    //│ !105  ┆ 0x483cf8942c76d86ed28a9849940abdea14952a551be84c38c844208e13d976d4 ┆ 65.109.64.57                                                                  ┆ Finland     │
    //│ !107  ┆ 0x8dd8b23892be142c2ec4b7e1427584c97403e570cde4e6d9a81d2b220c884af5 ┆ 3.66.156.72                                                                   ┆ Germany     │
    //└──────┴────────────────────────────────────────────────────────────────────┴───────────────────────────────────────────────────────────────────────────────┴─────────────┘

    [
        //"0xef191e643c2ed2d103de4109a506e020f5dde818cbc1ffe5e4ee36328cd22e9b",
        //"0xb4a4f1ef8b0702d85547dc444571a473f736e1205a86db36dad13815ad9bbbf6",
        //"0x86da6288dd50be1653aa2a8269acf881d3b4b414938ca7ab3b662c2dfeb794df",
        //"0xb5064f2e6d2d4887c836c8cbd90d23a75cd7afd4530e4a82f06bf528abcb30d6",
        //"0x81d41a31fb5d3f827d188511e6fca1ddbed6cfa0ffbd46fa6d84a7adfe59f356",
        //"0xc32f662cd9718f02d8a8e5628f8f642fa27cd9b5f457b406ed734901a4939e34",
        //"0xc807d24a6ac70599cc9b72c649eb6b00c34a0e9c704447c44d75fade07213a26",
        //"0x9a41af5c002f91dc29cb2843ab823e3d6cdaa6cc86e6331646a4f1328609a744",
        //"0x3d654d35d558d6381fe6a0484d81c4fed128cd8adc1ca7a1482c297da80ee039",
        //"0x1c6a051d9a5ec4cf4200bab21a794d3afe8259d625f6b78aa4d01ab4652327d5",
        //"0x6099edbe54f242bad50020dfd67646b1e46282999483e7064e70f02f7ea3c15",
        //"0xe1abb4ea11673aad3137052dce831eb20a1907815cb4a03ccac38ba7a9e988d3",
        //"0xbcb83e07eed46120a0143bd9861ea82495293228b9cd64cc43bda722edaeb7ad",
        //"0x302b69423fde5c7dd333ee50d48df16f076072e61f95e0f6db5685ee6c70d35b",
        //"0x85bec1dcb618004064ab52a594321e8f047bac53be8244c5d10ac136128e30f7",
        //"0xd2dc57e95669c9bf102cb5693fe8b2a8b10d330213d7ddd9fb417f86f183dc1d",
        //"0xe9f7d36334768683b85ec1df892dfd8fc389c8f6dbe0b890195d579a6e390c78",
        //"0x71debfa733376c4f2dc8970de8962931fe107c7dc1daad3bf189101bfd4adc4b",
        //"0x904bc056aa17a45c2715113e06cb42f5762581a7b839206f5348938db69425be",
        //"0xf2b6feeea4546648c257c7e33ff4f0aa8db3bb2949cca065c9db015478363458",
        // noise error???
        //"0x9bfd93ebaa1efd65515642942a607eeca53a0188c04c21ced646d2f0b9f551e8",
        "0x324df1e27c4129a58d73851ae0e9366064dc666a73e747051e203694a4cb257",
        "0x33f0c333f6fcb9d1ef54d54093f0482b31bb0bdcd34bfa446e7416ff930453c0",
        "0xef191e643c2ed2d103de4109a506e020f5dde818cbc1ffe5e4ee36328cd22e9b",
        "0x39b844cb561d2a04c800164465e2eb2c2ea520499d718c35492244346da07b5f",
        "0x3d654d35d558d6381fe6a0484d81c4fed128cd8adc1ca7a1482c297da80ee039",
        "0xda418307cff595ced2af5eb471ec11b1ad6a4907ef57d6e2eeb253bdd5bd9d0d",
        "0xb87476d8877cfc324a520cef4d0cd6e42e26ec3ce613a8127220e2e444561e16",
        "0xc44406bd36286223e7f2924ef48aa017b133dca73f2837b3692784cb046dda48",
        "0x6099edbe54f242bad50020dfd67646b1e46282999483e7064e70f02f7ea3c15",
        "0x29f3f84adbd1eba85bbbec33814924fddcdc9f51921d9cade0dd3b3fdf9170cd",
        "0x4fbd48a3ddf937873e0f50fb3a69172959f83feb46c4868bd3f7c45f1bdc2be0",
        "0x550fe3956e44112599b8410404e0977e74cc5bcb59a11b0706da734cd0f9beb2",
        "0x81d41a31fb5d3f827d188511e6fca1ddbed6cfa0ffbd46fa6d84a7adfe59f356",
        "0x8bf2201bf4cec31f736aacc0f44f29c014ecd8562975aecce19271ca179e8db5",
        "0x2537e9d461d311dc6f1dee690f093b4e353f472abc2493c64275a3d5572532cc",
        "0xee36c1068076b199cf537bf652b1e586216a7dfb7c3447ff40333c971717eee6",
        "0xf2b6feeea4546648c257c7e33ff4f0aa8db3bb2949cca065c9db015478363458",
        "0xc067ce10f1faa4812fe608e2d4b3e1fe0bb629e91cfff532cccc23803a624414",
        "0xe9f7d36334768683b85ec1df892dfd8fc389c8f6dbe0b890195d579a6e390c78",
        "0xe1abb4ea11673aad3137052dce831eb20a1907815cb4a03ccac38ba7a9e988d3",
        "0x18a3ef480b7883d05b917873a8b6d5f5a5f763372b650242ede89836a67cdce8",
        "0xf08bb83cf89d299e4cddee7ef987b850a35a399900ba22860cc1553da3682a84",
        "0x6c89d2927f78bb3b1743b4cf97d69777c2669cfbb11aff5bdaaa1083db3c3123",
        "0x8dd8b23892be142c2ec4b7e1427584c97403e570cde4e6d9a81d2b220c884af5",
        "0xc807d24a6ac70599cc9b72c649eb6b00c34a0e9c704447c44d75fade07213a26",
        "0xf05d2fe6618a3cb4258e94f2da24c1ee71ae4e7942b4f8b5836ad5267e59568f",
        "0xfc307ecd61ac025f5ec1828d2114d11b474b4d8a1b0c228c38614dc96b940048",
        "0xc29a231581f243ddcbf97de8b93c8aabb466dd6912ce5ba12579ff2f13cb8da5",
        "0xb225277ba076b6620447f7b10abb30911966eb66d25fb526b7f09c2227425a11",
        "0x6be5d9afa759b48140a69c737e9b066270146c7065455d04c2c76842c3e23d4",
        "0x8d5c9e82d612c390b5dea347918b6421146bb638724854300eed065cc4211a6e",
        "0x4cfeaf68aacfd0cd459c292837854ce058b9ba6c1cc35e8684b1a5030b1dc922",
        "0x447fc1fc130cf9245d9ca291785d0f8a00c3ebdc26b2e4c6165801b0af01c917",
        "0x483cf8942c76d86ed28a9849940abdea14952a551be84c38c844208e13d976d4",
        "0xb43fe1ef756cae4fefea16dad049fb0760b7d8593ace12527e302520d934d0d7",
        "0x9da88926fd4d773fd499fc41830a82fe9c9ff3508435e7a16b2d8f529e77cdda",
        "0x22dd12212d76bc5844b4a0605bf59fa56547f42a40fb42267e29adca5efecfc8",
        "0xd89481a2f4ff5598f8a6e83e67f21dba6a0df56a1bc35b9c67a2de6dce166b30",
        "0x209154f6e735562ff13b963b91588039a162fa95f409c068f917f27b762dfff8",
        "0xdc37463c5782469908d7ff43873390075980a8fa5b1c0770457c48f8891a5bff",
        "0xddf23883328d39d8cbb4fad97f0e4bd45827ea999b66aa2a1cffa76615b9ef01",
        "0xb840f1cfae5aaa4b50a11a6a23e2e859717e2f355fcd614592b219fc4d4e44a3",
        "0x4b2c524fc36698caf9d049b6088faa002cea3bbcbf5ed764f59768b4fe96efcf",
        "0xdb5247f859ce63dbe8940cf8773be722a60dcc594a8be9aca4b76abceb251b8e",
        "0xc92d451e6b5c1bca83d002c3dad857cb8fafb1e49ef5ddadf25c0f40f9d5728c",
        "0xefaec1debc26828f8142a71a906ba180cae526b0688f304a28e57fb66f2b9a5c",
        "0xb4a4f1ef8b0702d85547dc444571a473f736e1205a86db36dad13815ad9bbbf6",
        "0x9a41af5c002f91dc29cb2843ab823e3d6cdaa6cc86e6331646a4f1328609a744",
        "0x7425cde20dc18a8e5e169614c47c0e4435fd1b6c475efdefbdbbb73785d331e3",
        "0x1c6a051d9a5ec4cf4200bab21a794d3afe8259d625f6b78aa4d01ab4652327d5",
        "0xff3e9c10dd3781a1e0750a75ae9e5b04133cd7e8ca18b9936ffcf3b2a2538a49",
        "0x346361873c235e1a300fd67a2e8b7bd1ea3744001b2e0d1942317c4c8521f5dd",
        "0x401a881d9770f054a1b8cd6829f3d65a040bbe9b387ebcce50bddf0f853077d9",
        "0x63b0a7343c481fe176603e4e30cc5acc521c75d4310ce1b7b657c8168df66c66",
        "0x6a1d3313a58719cf5abc1aaee6b4b74bc7e2c69036a8a02f02eb4f3ab4a3d678",
        "0x85bec1dcb618004064ab52a594321e8f047bac53be8244c5d10ac136128e30f7",
        "0x9dfeca70acb3e01328d9dbbda4a6c23913fcd81ef25f74fab52cb2e96b0a1486",
        "0xc48683f9f684d2845c18e8d33d5f6c8581acfc1eb1c78b1de25562d136877e10",
        "0xc32f662cd9718f02d8a8e5628f8f642fa27cd9b5f457b406ed734901a4939e34",
        "0xff90c2afcd5c09b7414e4629d17899bcd763f8673c400a6d983fe15b98cb65d2",
        "0x22c25ea17f339761d512b588feb436a4784332f4bedfb3f17e25db7d8de0c94c",
        "0x371dfdee9fbecaa358683936c5946aa28393067fa60e1e185c4250da461e15cf",
        "0x71debfa733376c4f2dc8970de8962931fe107c7dc1daad3bf189101bfd4adc4b",
        "0x4c7c21084620f9e55f50f292d1803e5fbcf55c6d850b24b5e64efef08d2aa428",
        "0x9bfd93ebaa1efd65515642942a607eeca53a0188c04c21ced646d2f0b9f551e8",
        "0x30952a7b88b02028e7c6c7209722cc047acaef85d3d303f375d8ea89fc6ba0c6",
    ]
    .into_iter()
    .map(|x| x.to_string())
    .collect::<HashSet<String>>()
}
