use crate::discovery::KAD_MESSAGE_LEN;
use crate::node::peer_info::PeerInfo;
use crate::node::peer_key::Key;
use crate::protocol::protocol::{
    InnerKey, KadMessage, Message, MessageKey, Nodes, Peer, RPCBytes, RequestBytes, ResponseBytes,
    StoreKey, Value,
};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use crate::traits::routable::Routable;
use crate::impl_ByteRep;
use crate::utils::utils::ByteRep;

/// Implements ByteRep trait for RPC, Req & Resp structs so they can be
/// represented as a vector of bytes and/or returned to their struct form
/// with one simple function call.
impl_ByteRep!(for RPC, Req, Resp);


/// RPC is an enum of the different types of remote procedure calls that a
/// kademlia instance may receive from or send to peers in the network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RPC {
    Ping,
    NewPeer(Peer),
    Store(StoreKey, Value),
    FindNode(Peer),
    FindValue(Value),
    Nodes(Nodes),
    Value(Value),
    Saved(StoreKey),
    Pong(Peer),
}

/// A struct that contains an RPC request, the sender of the request
/// and an ID for the request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Req {
    pub id: InnerKey,
    pub sender: Peer,
    pub payload: RPCBytes,
}

/// A struct that contains an RPC response, the original request that
/// we are responding to and the original receiver of the request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resp {
    pub request: RequestBytes,
    pub receiver: Peer,
    pub payload: RPCBytes,
}

impl Req {

    /// Breaks down a Req struct into it's 3 individual components
    /// and returns them from byte/inner representation to the struct
    /// representation.
    /// 
    /// TODO: Conver this impl and the one for Resp into a trait
    pub fn to_components(&self) -> (MessageKey, Option<PeerInfo>, Option<RPC>) {
        let rpc = RPC::from_bytes(&self.payload);
        let sender = PeerInfo::from_bytes(&self.sender);
        let id = MessageKey::from_inner(self.id);
        (id, sender, rpc)
    }
}

impl Resp {
    /// Breaks down a Resp struct into it's 3 individual components
    /// and returns them from byte/inner representation to the struct
    /// representation.
    /// 
    /// TODO: Conver this impl and the one for Resp into a trait
    pub fn to_components(&self) -> (Option<Req>, Option<PeerInfo>, Option<RPC>) {
        let rpc = RPC::from_bytes(&self.payload);
        let receiver = PeerInfo::from_bytes(&self.receiver);
        let req = Req::from_bytes(&self.request);

        (req, receiver, rpc)
    }
}

pub trait Request {}
pub trait Response {}

#[macro_export]
macro_rules! impl_Request {
    (for $($t:ty), +) => {
        $(impl Request for $t {})*
    };
}

#[macro_export]
macro_rules! impl_Response {
    (for $($t:ty), +) => {
        $(impl Response for $t {})*
    };
}

impl_Request!(for Req, RPC, KadMessage);
impl_Response!(for Resp, RPC, KadMessage);
