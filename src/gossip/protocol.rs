use std::net::SocketAddr;
use crate::impl_ByteRep;
use crate::utils::utils::ByteRep;
use serde::{Serialize, Deserialize};
use crate::protocol::protocol::{InnerKey, MessageData};

impl_ByteRep!(for GossipMessage);

/// The gossip message 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipMessage {
    pub id: InnerKey,
    pub data: MessageData,
    pub sender: SocketAddr,
}