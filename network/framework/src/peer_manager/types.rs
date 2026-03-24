// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    peer::DisconnectReason,
    peer_manager::PeerManagerError,
    protocols::{direct_send::Message, rpc::OutboundRpcRequest},
    transport::{Connection, ConnectionMetadata},
};
use aptos_config::network_id::NetworkId;
use aptos_types::{network_address::NetworkAddress, PeerId};
use futures::channel::oneshot;
use serde::Serialize;
use std::fmt;

use aptos_crypto::x25519;

/// Request received by PeerManager from upstream actors.
#[derive(Debug, Serialize)]
pub enum PeerManagerRequest {
    /// Send an RPC request to a remote peer.
    SendRpc(PeerId, #[serde(skip)] OutboundRpcRequest),
    /// Fire-and-forget style message send to a remote peer.
    SendDirectSend(PeerId, #[serde(skip)] Message),
}

#[derive(Debug, Serialize)]
pub enum ConnectionRequest {
    /// oinori: DialPeer now carries an ephemeral identity (private key + PeerId)
    /// for the connection pool identity rotation mechanism.
    DialPeer(
        x25519::PrivateKey, // oinori: ephemeral private key for Noise handshake
        PeerId,             // oinori: ephemeral PeerId derived from the key above
        PeerId,             // target peer to dial
        NetworkAddress,
        #[serde(skip)] oneshot::Sender<Result<(), PeerManagerError>>,
    ),
    DisconnectPeer(
        PeerId,
        #[serde(skip)] oneshot::Sender<Result<(), PeerManagerError>>,
    ),
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub enum ConnectionNotification {
    /// Connection with a new peer has been established.
    NewPeer(ConnectionMetadata, NetworkId),
    /// Connection to a peer has been terminated. This could have been triggered from either end.
    LostPeer(ConnectionMetadata, NetworkId),
}

impl fmt::Debug for ConnectionNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for ConnectionNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionNotification::NewPeer(metadata, network_id) => {
                write!(f, "[{},{}]", metadata, network_id)
            },
            ConnectionNotification::LostPeer(metadata, network_id) => {
                write!(f, "[{},{}]", metadata, network_id)
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub enum TransportNotification<TSocket> {
    NewConnection(#[serde(skip)] Connection<TSocket>),
    Disconnected(ConnectionMetadata, DisconnectReason),
}
