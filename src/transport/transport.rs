use crate::gd_udp::gd_udp::GDUdp;
use crate::protocol::protocol::{packetize, split_into_packets, AckMessage, Header, Message, MessageKey, Packets, Packet};
use crate::utils::utils::ByteRep;
use log::info;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::mpsc::Receiver;

/// A struct for managing the transport layer in a p2p network
/// contains a GDUdp struct for sending reliable messages over UDP
/// an incoming acknowledgement receiver to receiving return receipts from peers
/// and an outgoing message receive to get messages to send from other threads
#[derive(Debug)]
pub struct Transport {
    gd_udp: GDUdp,
    ia_rx: Receiver<AckMessage>,
    om_rx: Receiver<(SocketAddr, Message)>
}

impl Transport {
    /// Creates a new instance of the Transport struct
    ///
    /// # Arguments
    ///
    /// * addr - the local socket address
    /// * ia_rx - the incoming acknowledgement receiver
    /// * om_rx - the outgoing message receiver
    pub fn new(
        addr: SocketAddr,
        ia_rx: Receiver<AckMessage>,
        om_rx: Receiver<(SocketAddr, Message)>
    ) -> Transport {
        Transport {
            gd_udp: GDUdp::new(addr),
            ia_rx,
            om_rx
        }
    }

    /// Handles incomingi acknowledgements
    pub fn incoming_ack(&mut self) {
        let res = self.ia_rx.try_recv();
        match res {
            Ok(ack) => {
                let exists = self.gd_udp.outbox.contains_key(&ack.packet_id);
                if exists {
                    self.gd_udp
                        .process_ack(ack.packet_id, ack.packet_number, ack.src);
                };
            }
            Err(_) => {}
        }
    }

    /// Handles and sends outgoing messages
    ///
    /// # Arguments
    ///
    /// * sock - The UDP socket for the message to be sent out on.
    ///
    pub fn outgoing_msg(&mut self, sock: &UdpSocket) {
        let res = self.om_rx.try_recv();
        let res1=res.clone();
        if res1.is_ok(){
            if res1.unwrap().1.head==Header::RaptorQGossip{
                println!("Gossip header caught");
            }
        }
        match res {
            Ok((src, msg)) => match msg.head {
                Header::Ack => {
                    let packets_id = MessageKey::rand().inner();
                    let packets = packetize(msg.as_bytes().unwrap().clone(), packets_id, 0u8,false);
                    packets.iter().for_each(|packet| {
                        if let Err(_) = sock.send_to(&packet.as_bytes().unwrap(), src) {}
                    });
                }
                Header::RaptorQGossip => {
                    let ip = self.gd_udp.addr.to_string();
                    let split_local: Vec<&str> = ip.split(":").collect();
                    let peer = src.to_string();
                    let split_peer: Vec<&str> = peer.split(":").collect();
                    let packets_id = MessageKey::rand().inner();
                    let packets =
                        split_into_packets(&msg.msg, packets_id, 3000);
                    let packets: Packets = packets
                        .iter()
                        .enumerate()
                        .map(|(idx, packet)| {
                            let hex_string = hex::encode(&packet);
                            Packet {
                                id:packets_id,
                                n: idx + 1,
                                total_n: packets.len(),
                                bytes: hex_string,
                                ret: 0,
                                is_raptor_q_packet: true
                            }
                        })
                        .collect();
                    packets.iter().for_each(|packet| {
                        if split_local[0] == split_peer[0] {
                            let new_ip = "127.0.0.1".parse::<Ipv4Addr>().unwrap();
                            let port = split_peer[1].parse::<u32>().unwrap();
                            let new_src = format!("{:?}:{:?}", new_ip, port)
                                .parse::<SocketAddr>()
                                .unwrap();
                            if let Some(bytes) = packet.as_bytes() {
                                if let Err(e) = sock.send_to(&bytes, new_src) {
                                    info!("Error sending packet to {:?}:\n{:?}", new_src, e)
                                }
                            }

                        } else {
                            if let Some(bytes) = packet.as_bytes() {
                                if let Err(e) = sock.send_to(&bytes, src) {
                                    info!("Error sending packet to {:?}:\n{:?}", src, e)
                                }
                            }
                        }
                    });
                }
                _ => {
                    let ip = self.gd_udp.addr.to_string();
                    let split_local: Vec<&str> = ip.split(":").collect();
                    let peer = src.to_string();
                    let split_peer: Vec<&str> = peer.split(":").collect();

                    let packets_id = MessageKey::rand().inner();
                    let packets = packetize(msg.as_bytes().unwrap().clone(), packets_id, 1u8,false);
                    packets.iter().for_each(|packet| {
                        if split_local[0] == split_peer[0] {
                            let new_ip = "127.0.0.1".parse::<Ipv4Addr>().unwrap();
                            let port = split_peer[1].parse::<u32>().unwrap();
                            let new_src = format!("{:?}:{:?}", new_ip, port)
                                .parse::<SocketAddr>()
                                .unwrap();
                            self.gd_udp.send_reliable(&new_src, packet, &sock);
                        } else {
                            self.gd_udp.send_reliable(&src, packet, &sock);
                        }
                    });
                }
            },
            Err(_) => {}
        }
    }

    /// Checks if its time to maintain the GDUDP instance cointained
    /// in the Tranpsort instance.
    pub fn check_time_elapsed(&mut self, sock: &UdpSocket) {
        self.gd_udp.check_time_elapsed(sock)
    }
}
