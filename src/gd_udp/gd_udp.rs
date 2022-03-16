use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::protocol::protocol::{InnerKey, Packet, AddressBytes, Packets};
use crate::utils::utils::ByteRep;
use log::info;

/// A pseudo-guaranteed deliver wrapper for UDP sockets to ensure that
/// packets are either delivered, or are resent to the destination.
/// Recieves the local socket address, a cache of messages received
/// an outbox for messages sent that require a return receipt.
/// The outbox is the main field in the struct, each message sent is stored with the
/// id and a hashmap of key == packet number, value = quaduple of a set of destinations
/// set of returned receipts, the packets, and the number attempts. The timer is used to to 
/// determine whether enough time has passed to attempt to resend unacknowledged packets.
#[derive(Debug, Clone)]
pub struct GDUdp {
    pub addr: SocketAddr,
    pub message_cache: HashSet<InnerKey>,
    pub outbox: HashMap<InnerKey, HashMap<usize, (HashSet<SocketAddr>, HashSet<SocketAddr>, Packet, usize)>>,
    pub timer: Instant,
    pub log: String,
}

impl GDUdp {
    /// The 3 constants are used by the GDUDP to determine if a message
    /// needs to be resent.
    pub const MAINTENANCE: Duration = Duration::from_millis(300);
    pub const RETURN_RECEIPT: u8 = 1u8;
    pub const NO_RETURN_RECEIPT: u8 = 0u8;

    /// Generates a new GDUDP instance given a socket address
    /// 
    /// # Arguments
    /// 
    /// * addr - the local nodes socket address
    /// 
    pub fn new(addr: SocketAddr) -> GDUdp {
        GDUdp {
            addr,
            message_cache: HashSet::new(),
            outbox: HashMap::new(),
            timer: Instant::now(),
            log: "log.log".to_string(),
        }
    }

    /// Loops through the outbox and resends packets that haven't been acknowldged
    /// and tracks the number of attempts.
    /// TODO: 
    /// 
    /// add a GDUDPConfig struct that contains config information like number of attempts
    /// before giving up on a given packet, maintenance duration, etc
    /// 
    /// # Arguments
    /// 
    /// * sock - the UDP socket for the local node used to resend unacknowldged packets.
    pub fn maintain(&mut self, sock: &UdpSocket) {
        self.outbox.retain(|_, map| {
            map.retain(|_, (sent_set, ack_set, _, attempts)| sent_set != ack_set && *attempts < 5);
            !map.is_empty()
        });
        
        self.outbox.clone().iter().for_each(|(_, map)| {
            map.iter().for_each(|(_, (sent_set, ack_set, packet, attempts))| {
                if *attempts < 5 {
                    let resend: HashSet<_> = sent_set.difference(ack_set).collect();
                    resend.iter().for_each(|peer| {
                        self.send_reliable(peer, &packet, sock);
                    });
                }
            });
        });
    }

    /// Receives incoming messages from a given udp socket
    /// used only in apps that choose not to implement a transport layer
    /// it is highly recommended that a transport layer is used for this crate to integrate with
    /// p2p apps
    /// 
    /// # Arguments
    /// 
    /// * sock - a UDP socket to receive messages on
    /// * buf - a buffer to write incoming message bytes to.
    pub fn recv_from(
        &mut self,
        sock: Arc<UdpSocket>,
        buf: &mut [u8],
    ) -> Result<(usize, SocketAddr), std::io::Error> {
        match sock.recv_from(buf) {
            Err(e) => return Err(e),
            Ok((amt, src)) => return Ok((amt, src)),
        }
    }

    /// Checks the amount of time that has passed since the last maintenance of the outbox
    /// and calls maintain if it's time to maintain the outbox.
    /// 
    /// # Arguments
    /// 
    /// * sock - UDP socket to pass into the maintain function call
    ///  
    pub fn check_time_elapsed(&mut self, sock: &UdpSocket) {
        let now = Instant::now();
        let time_elapsed = now.duration_since(self.timer);
        let cloned_sock = sock.try_clone().expect("Unable to clone socket");

        if time_elapsed >= GDUdp::MAINTENANCE {
            self.maintain(&cloned_sock);
            self.timer = Instant::now()
        }
    }

    /// Processes an acknowldgement message
    /// 
    /// # Arguments
    /// 
    /// * id - the InnerKey of the message being acknowledged
    /// * packet_number - the packet number being acknowledged
    /// * src - the node that's acknowledging receipt of the packet
    /// 
    pub fn process_ack(&mut self, id: InnerKey, packet_number: usize, src: AddressBytes) {
        let src = String::from_utf8_lossy(&src);
        let src = src.parse().expect("Unable to parse socket address");
        if let Some(map) = self.outbox.get_mut(&id) {
            if let Some((_, ack_set, _, _)) = map.get_mut(&packet_number) {
                ack_set.insert(src);
            }
        }
    }

    /// Sends a message with a return receipt requested to a peer in the network
    /// 
    /// # Arguments
    /// 
    /// * peer - the destination address
    /// * packet - the packet to send
    /// * sock - the UDP socket to send the message out on
    /// 
    pub fn send_reliable(
        &mut self,
        peer: &SocketAddr,
        packet: &Packet,
        sock: &UdpSocket,
    ) {
        if let Some(map) = self.outbox.get_mut(&packet.id) {
            if let Some((sent_set, _, _, attempts)) = map.get_mut(&packet.n) {
                sent_set.insert(peer.clone());
                *attempts += 1;
            } else {
                let mut sent_set = HashSet::new();
                let ack_set: HashSet<SocketAddr> = HashSet::new();
                sent_set.insert(peer.clone());
                let attempts = 1;
                map.insert(packet.n, (sent_set, ack_set, packet.clone(), attempts));
            }
        } else {
            let mut map = HashMap::new();
            let mut sent_set = HashSet::new();
            let ack_set: HashSet<SocketAddr> = HashSet::new();
            sent_set.insert(peer.clone());
            let attempts = 1;
            map.insert(packet.n, (sent_set, ack_set, packet.clone(), attempts));
            self.outbox.insert(packet.id, map);
        }
        if let Some(bytes) = packet.as_bytes() {
            if let Err(e) = sock.send_to(&bytes, peer) {
                info!("Error sending packet to {:?}:\n{:?}", peer, e)
            } else {
                info!("Sent packet {:?} to {:?}", &packet.id, peer)
            }
        }
    }

    /// Sends an acknowledgement message to the sender of a packet with a return receipt requested
    /// 
    /// # Arguments
    /// 
    /// * sock - the socket to send the packets on
    /// * peer - the destination address
    /// * packets - a vector of packets
    pub fn ack(&mut self, sock: &UdpSocket, peer: &SocketAddr, packets: Packets) {
        packets.iter().for_each(|packet| {
            if let Some(bytes) = packet.as_bytes() {
                sock.send_to(&bytes, peer)
                    .expect("Unable to send message to peer");
            }
        })
    }

}
