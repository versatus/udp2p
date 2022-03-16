use crate::gd_udp::gd_udp::GDUdp;
use crate::protocol::protocol::{packetize, AckMessage, Header, Message, MessageKey};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::Receiver;
use crate::utils::utils::ByteRep;

/// A struct for managing the transport layer in a p2p network
/// contains a GDUdp struct for sending reliable messages over UDP
/// an incoming acknowledgement receiver to receiving return receipts from peers
/// and an outgoing message receive to get messages to send from other threads
#[derive(Debug)]
pub struct Transport {
    gd_udp: GDUdp,
    ia_rx: Receiver<AckMessage>,
    om_rx: Receiver<(SocketAddr, Message)>,
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
        om_rx: Receiver<(SocketAddr, Message)>,
    ) -> Transport {
        Transport {
            gd_udp: GDUdp::new(addr),
            ia_rx,
            om_rx,
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
        match res {
            Ok((src, msg)) => match msg.head {
                Header::Ack => {
                    let packets_id = MessageKey::rand().inner();
                    let packets = packetize(msg.as_bytes().unwrap().clone(), packets_id, 0u8);
                    packets.iter().for_each(|packet| {
                        if let Err(_) = sock.send_to(&packet.as_bytes().unwrap(), src) {}
                    });
                }
                _ => {
                    println!("Sending message to: {:?}", src);
                    let packets_id = MessageKey::rand().inner();
                    let packets = packetize(msg.as_bytes().unwrap().clone(), packets_id, 1u8);
                    packets.iter().for_each(|packet| {
                        self.gd_udp.send_reliable(&src, packet, &sock);
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
