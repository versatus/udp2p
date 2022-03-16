use std::net::SocketAddr;
use crate::utils::utils::Distance;

pub trait Routable: Distance {
    type Id;
    type Key;

    fn get_id(&self) -> Self::Id;
    fn get_key(&self) -> Self::Key;
    fn get_address(&self) -> SocketAddr;
}