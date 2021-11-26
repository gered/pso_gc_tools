use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Formatter};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use etherparse::{IpHeader, PacketHeaders};
use pcap::{Capture, Offline};
use pretty_hex::*;
use thiserror::Error;

use psoutils::encryption::{Crypter, GCCrypter};
use psoutils::packets::init::InitEncryptionPacket;
use psoutils::packets::{GenericPacket, PacketHeader};

fn timeval_to_dt(ts: &::libc::timeval) -> DateTime<Utc> {
    Utc.timestamp(ts.tv_sec, ts.tv_usec as u32 * 1000)
}

#[derive(Error, Debug)]
enum TcpDataPacketError {
    #[error("No IpHeader in packet")]
    NoIpHeader,

    #[error("No TransportHeader in packet")]
    NoTransportHeader,

    #[error("No TcpHeader in packet")]
    NoTcpHeader,
}

struct TcpDataPacket {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub tcp_fin: bool,
    pub tcp_rst: bool,
    pub data: Box<[u8]>,
}

impl TcpDataPacket {
    pub fn as_init_encryption_packet(&self) -> Option<InitEncryptionPacket> {
        let mut reader = Cursor::new(&self.data);
        if let Ok(packet) = InitEncryptionPacket::from_bytes(&mut reader) {
            Some(packet)
        } else {
            None
        }
    }
}

impl<'a> TryFrom<PacketHeaders<'a>> for TcpDataPacket {
    type Error = TcpDataPacketError;

    fn try_from(value: PacketHeaders) -> Result<Self, Self::Error> {
        let source_ip: IpAddr;
        let source_port: u16;
        let destination_ip: IpAddr;
        let destination_port: u16;
        let payload_len: usize;
        let data_offset: usize;
        let tcp_fin: bool;
        let tcp_rst: bool;

        if let Some(ip_header) = &value.ip {
            let (source, destination, len) = match ip_header {
                IpHeader::Version4(ipv4_header) => (
                    IpAddr::from(ipv4_header.source),
                    IpAddr::from(ipv4_header.destination),
                    ipv4_header.payload_len,
                ),
                IpHeader::Version6(ipv6_header) => (
                    IpAddr::from(ipv6_header.source),
                    IpAddr::from(ipv6_header.destination),
                    ipv6_header.payload_length,
                ),
            };
            source_ip = source;
            destination_ip = destination;
            payload_len = len as usize;
        } else {
            return Err(TcpDataPacketError::NoIpHeader);
        }

        if let Some(transport_header) = value.transport {
            if let Some(tcp_header) = transport_header.tcp() {
                source_port = tcp_header.source_port;
                destination_port = tcp_header.destination_port;
                data_offset = tcp_header.header_len() as usize;
                tcp_fin = tcp_header.fin;
                tcp_rst = tcp_header.rst;
            } else {
                return Err(TcpDataPacketError::NoTcpHeader);
            }
        } else {
            return Err(TcpDataPacketError::NoTransportHeader);
        }

        // this ensures we don't get any padding bytes that might have been added onto the end ...
        let data: Box<[u8]> = value.payload[0..(payload_len - data_offset)].into();

        Ok(TcpDataPacket {
            source: SocketAddr::new(source_ip, source_port),
            destination: SocketAddr::new(destination_ip, destination_port),
            tcp_fin,
            tcp_rst,
            data,
        })
    }
}

impl Debug for TcpDataPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TcpDataPacket {{ source={}, destination={}, length={} }}",
            self.source,
            self.destination,
            self.data.len()
        )?;
        Ok(())
    }
}

struct Peer {
    crypter: Option<GCCrypter>,
    address: SocketAddr,
    raw_buffer: Vec<u8>,
    decrypted_buffer: Vec<u8>,
    packets: Vec<GenericPacket>,
}

impl Peer {
    pub fn new(address: SocketAddr) -> Peer {
        Peer {
            crypter: None,
            address,
            raw_buffer: Vec::new(),
            decrypted_buffer: Vec::new(),
            packets: Vec::new(),
        }
    }

    pub fn init_pso_session(&mut self, crypt_key: u32) {
        self.crypter = Some(GCCrypter::new(crypt_key));
        self.raw_buffer.clear();
        self.decrypted_buffer.clear();
    }

    pub fn push_pso_packet(&mut self, packet: GenericPacket) {
        self.packets.push(packet)
    }

    pub fn process_packet(&mut self, packet: TcpDataPacket) -> Result<()> {
        if self.address != packet.source {
            return Err(anyhow!(
                "This Peer({}) cannot process TcpDataPacket originating from different source: {}",
                self.address,
                packet.source
            ));
        }

        // don't begin collecting data unless we're prepared to decrypt that data ...
        if let Some(crypter) = &mut self.crypter {
            // incoming bytes get added to the raw (encrypted) buffer first ...
            self.raw_buffer.append(&mut packet.data.into_vec());

            // we should only be decrypting dword-sized bits of data (based on the way that the
            // encryption algorithm works) so if we have that much data, lets go ahead and decrypt that
            // much and move those bytes over to the decrypted buffer ...
            if self.raw_buffer.len() >= 4 {
                let length_to_decrypt = self.raw_buffer.len() - (self.raw_buffer.len() & 3); // dword-sized length only!
                let mut bytes_to_decrypt: Vec<u8> =
                    self.raw_buffer.drain(0..length_to_decrypt).collect();
                crypter.crypt(&mut bytes_to_decrypt);
                self.decrypted_buffer.append(&mut bytes_to_decrypt);
            }
        }

        // try to extract as many complete packets out of the decrypted buffer as we can
        while self.decrypted_buffer.len() >= PacketHeader::header_size() {
            // if we have at least enough bytes for a PacketHeader available, read one out and figure
            // out if we have enough remaining bytes for the full packet that this header is for
            let mut reader = &self.decrypted_buffer[0..PacketHeader::header_size()];
            if let Ok(header) = PacketHeader::from_bytes(&mut reader) {
                if self.decrypted_buffer.len() >= header.size as usize {
                    // the buffer has enough bytes for this entire packet. read it out and add it
                    // to our internal list of reconstructed packets
                    let packet_length = header.size as usize;
                    let packet_bytes: Vec<u8> =
                        self.decrypted_buffer.drain(0..packet_length).collect();
                    let mut reader = Cursor::new(packet_bytes);
                    self.packets.push(GenericPacket::from_bytes(&mut reader)?);
                } else {
                    // unable to read the full packet with the bytes currently in the decrypted
                    // buffer ... so we'll need to try again later after receiving some more data
                    break;
                }
            }
        }

        Ok(())
    }
}

impl Iterator for Peer {
    type Item = GenericPacket;

    fn next(&mut self) -> Option<Self::Item> {
        if self.packets.is_empty() {
            return None;
        } else {
            Some(self.packets.remove(0))
        }
    }
}

impl Debug for Peer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Peer {{ address={} }}", self.address)?;
        Ok(())
    }
}

struct Session {
    peers: HashMap<SocketAddr, Peer>,
}

impl Session {
    pub fn new() -> Session {
        Session {
            peers: HashMap::new(),
        }
    }

    pub fn get_peer(&mut self, address: SocketAddr) -> Option<&mut Peer> {
        self.peers.get_mut(&address)
    }

    fn get_or_create_peer(&mut self, address: SocketAddr) -> &mut Peer {
        if self.peers.contains_key(&address) {
            self.peers.get_mut(&address).unwrap()
        } else {
            println!("Encountered new peer: {}\n", address);
            let new_peer = Peer::new(address);
            self.peers.insert(address, new_peer);
            self.get_or_create_peer(address)
        }
    }

    pub fn process_packet(&mut self, packet: TcpDataPacket) -> Result<()> {
        if packet.tcp_rst {
            println!(
                "Encountered TCP RST. Removing peers {} and {}.\n",
                packet.source, packet.destination
            );
            self.peers.remove(&packet.source);
            self.peers.remove(&packet.destination);
        } else if packet.tcp_fin {
            println!("Peer {} sent TCP FIN. Removing peer.\n", packet.source);
            self.peers.remove(&packet.source);
        } else if let Some(init_packet) = packet.as_init_encryption_packet() {
            println!(
                "Encountered InitEncryptionPacket sent from peer {}. Starting new session.",
                packet.source
            );

            // the "init packet" indicates the start of a PSO client/server session. this could
            // occur multiple times within the same pcap file as a client moves between different
            // servers (e.g. from login server to ship server, switching between ships, etc).

            println!(
                "Treating peer {} as the client, setting client decryption key: {:#010x}",
                packet.destination,
                init_packet.client_key()
            );

            let client = self.get_or_create_peer(packet.destination);
            client.init_pso_session(init_packet.client_key);

            println!(
                "Treating peer {} as the server, setting server decryption key: {:#010x}",
                packet.source,
                init_packet.server_key()
            );

            let server = self.get_or_create_peer(packet.source);
            server.init_pso_session(init_packet.server_key);
            server.push_pso_packet(
                init_packet
                    .try_into()
                    .context("Failed to convert InitEncryptionPacket into GenericPacket")?,
            );

            println!();
        } else {
            // process the packet via the peer it was sent from
            let peer = self.get_or_create_peer(packet.source);
            peer.process_packet(packet)
                .with_context(|| format!("Failed to process packet for peer {:?}", peer))?;
        }

        Ok(())
    }
}

pub fn analyze(path: &Path) -> Result<()> {
    println!("Opening capture file: {}", path.to_string_lossy());

    let mut cap: Capture<Offline> = Capture::from_file(path)
        .with_context(|| format!("Failed to open capture file: {:?}", path))?
        .into();
    cap.filter("tcp")
        .context("Failed to apply 'tcp' filter to opened capture")?;

    let mut session = Session::new();

    let hex_cfg = HexConfig {
        title: false,
        width: 16,
        group: 0,
        ..HexConfig::default()
    };

    println!("Beginning analysis ...\n");

    while let Ok(raw_packet) = cap.next() {
        if let Ok(decoded_packet) = PacketHeaders::from_ethernet_slice(raw_packet.data) {
            if let Ok(our_packet) = TcpDataPacket::try_from(decoded_packet) {
                let dt = timeval_to_dt(&raw_packet.header.ts);

                println!("<<<<< {} >>>>> - {:?}\n", dt, our_packet);

                let peer_address = our_packet.source;

                session
                    .process_packet(our_packet)
                    .context("Session failed to process packet")?;

                if let Some(peer) = session.get_peer(peer_address) {
                    while let Some(pso_packet) = peer.next() {
                        println!(
                            "id=0x{:02x}, flags=0x{:02x}, size={} (0x{2:04x})",
                            pso_packet.header.id(),
                            pso_packet.header.flags(),
                            pso_packet.header.size()
                        );
                        if pso_packet.body.is_empty() {
                            println!("<No data>");
                        } else {
                            println!("{:?}", pso_packet.body.hex_conf(hex_cfg));
                        }
                        println!();
                    }
                }
            } else {
                println!(
                    "*** TcpDataPacket::try_from failed for packet={:?}",
                    raw_packet.header
                );
            }
        } else {
            println!(
                "*** PacketHeaders::from_ethernet_slice failed for packet={:?}",
                raw_packet.header
            );
        }
    }

    Ok(())
}
