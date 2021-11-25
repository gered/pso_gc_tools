use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::io::{Cursor, Read};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use anyhow::{anyhow, Result};
use etherparse::{IpHeader, PacketHeaders};
use pcap::{Capture, Offline};
use pretty_hex::*;
use thiserror::Error;

use psoutils::encryption::{Crypter, GCCrypter};
use psoutils::packets::init::InitEncryptionPacket;
use psoutils::packets::{GenericPacket, PacketHeader};

fn take_buffer_bytes(buffer: &mut Vec<u8>, length: usize) -> Box<[u8]> {
    let bytes: Box<[u8]> = buffer[0..length].into();
    let remaining_length = buffer.len() - length;
    buffer.copy_within(length.., 0);
    buffer.truncate(remaining_length);
    bytes
}

fn peek_buffer_bytes(buffer: &Vec<u8>, length: usize) -> Box<[u8]> {
    let length = if buffer.len() > length {
        buffer.len()
    } else {
        length
    };
    buffer[0..length].into()
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

#[derive(Debug)]
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

#[derive(Debug)]
struct PsoPacket {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub packet: GenericPacket,
}

impl PsoPacket {
    pub fn new<T: Read>(
        source: SocketAddr,
        destination: SocketAddr,
        header: PacketHeader,
        reader: &mut T,
    ) -> Result<PsoPacket> {
        let mut raw_data = vec![0u8; header.size as usize - PacketHeader::header_size()];
        reader.read_exact(&mut raw_data)?;

        Ok(PsoPacket {
            source,
            destination,
            packet: GenericPacket::new(header, raw_data.into()),
        })
    }
}

struct Peer {
    crypter: GCCrypter,
    address: SocketAddr,
    encrypted_buffer: Vec<u8>,
    decrypted_buffer: Vec<u8>,
}

impl Peer {
    pub fn new(crypt_key: u32, address: SocketAddr) -> Peer {
        Peer {
            crypter: GCCrypter::new(crypt_key),
            address,
            encrypted_buffer: Vec::new(),
            decrypted_buffer: Vec::new(),
        }
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }

    pub fn process_packet(&mut self, packet: TcpDataPacket) -> Result<Option<PsoPacket>> {
        if self.address != packet.source {
            return Err(anyhow!(
                "This Peer({}) cannot process TcpDataPacket originating from different source: {}",
                self.address,
                packet.source
            ));
        }

        // incoming bytes get added to the encrypted buffer first ...
        self.encrypted_buffer.append(&mut packet.data.into_vec());

        // we should only be decrypting dword-sized bits of data (based on the way that the
        // encryption algorithm works) so if we have that much data, lets go ahead and decrypt that
        // much and move those bytes over to the decrypted buffer ...
        if self.encrypted_buffer.len() >= 4 {
            let length_to_decrypt = self.encrypted_buffer.len() - (self.encrypted_buffer.len() & 3);
            let mut bytes_to_decrypt: Vec<u8> =
                self.encrypted_buffer.drain(0..length_to_decrypt).collect();
            self.crypter.crypt(&mut bytes_to_decrypt);
            self.decrypted_buffer.append(&mut bytes_to_decrypt);
        }

        // try to read a PacketHeader out of the decrypted buffer, and if successful, read out the
        // entire packet data if we have enough bytes available in the decrypted buffer
        // (if either of these fail, we need to leave the current decrypted buffer alone for now)
        if self.decrypted_buffer.len() >= PacketHeader::header_size() {
            let mut reader = Cursor::new(&self.decrypted_buffer);
            if let Ok(header) = PacketHeader::from_bytes(&mut reader) {
                if self.decrypted_buffer.len() >= header.size as usize {
                    let pso_packet =
                        PsoPacket::new(packet.source, packet.destination, header, &mut reader)?;
                    // need to also remove the entire packet's bytes from the front of the buffer
                    self.decrypted_buffer.drain(0..header.size as usize);
                    return Ok(Some(pso_packet));
                }
            }
        }

        Ok(None)
    }
}

struct Context {
    is_pso_session_inited: bool,
    peers: HashMap<SocketAddr, Peer>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            is_pso_session_inited: false,
            peers: HashMap::new(),
        }
    }

    pub fn is_pso_session_inited(&self) -> bool {
        self.is_pso_session_inited
    }

    pub fn process(&mut self, packet: TcpDataPacket) -> Result<Option<PsoPacket>> {
        if packet.tcp_rst {
            println!("Encountered TCP RST. Resetting session. Expecting to encounter new peers and InitEncryptionPacket next ...");
            self.peers.clear();
            self.is_pso_session_inited = false;
            Ok(None)
        } else if packet.tcp_fin {
            println!("Peer {} sent TCP FIN. Resetting session. Expecting to encounter new peers and InitEncryptionPacket next ...", packet.source);
            self.peers.clear();
            self.is_pso_session_inited = false;
            Ok(None)
        } else if let Some(init_packet) = packet.as_init_encryption_packet() {
            println!("Encountered InitEncryptionPacket. Starting new session ...");
            // the "init packet" indicates the start of a PSO client/server session. this could occur
            // multiple times within the same pcap file as a client moves between different servers
            // (e.g. login server, ship server, ...)

            self.peers.clear();
            let server = Peer::new(init_packet.server_key, packet.source);
            let client = Peer::new(init_packet.client_key, packet.destination);
            self.peers.insert(packet.source, server);
            self.peers.insert(packet.destination, client);

            self.is_pso_session_inited = true;

            Ok(Some(PsoPacket {
                source: packet.source,
                destination: packet.destination,
                packet: init_packet.try_into()?,
            }))
        } else if !self.peers.is_empty() {
            // otherwise, if we have a set of peers already (because of a previous init packet)
            // then we can process this packet using the peer it was sent from

            let peer = match self.peers.get_mut(&packet.source) {
                None => return Err(anyhow!("No matching peer for {} ... ?", packet.source)),
                Some(peer) => peer,
            };
            Ok(peer.process_packet(packet)?)
        } else {
            // this would occur only if no init packet has been found yet. as such, this is
            // probably non-PSO packet stuff we don't care about
            Ok(None)
        }
    }
}

pub fn analyze(path: &Path) -> Result<()> {
    println!("Opening capture file: {}", path.to_string_lossy());

    let mut cap: Capture<Offline> = Capture::from_file(path)?.into();
    cap.filter("tcp");

    let mut context = Context::new();

    let hex_cfg = HexConfig {
        title: false,
        width: 16,
        group: 0,
        ..HexConfig::default()
    };

    while let Ok(raw_packet) = cap.next() {
        if let Ok(decoded_packet) = PacketHeaders::from_ethernet_slice(raw_packet.data) {
            if let Ok(mut our_packet) = TcpDataPacket::try_from(decoded_packet) {
                println!(
                    ">>>> packet - ts: {}.{}, from: {:?}, to: {:?}, length: {}\n",
                    raw_packet.header.ts.tv_sec,
                    raw_packet.header.ts.tv_usec,
                    our_packet.source,
                    our_packet.destination,
                    our_packet.data.len()
                );
                if let Some(pso_packet) = context.process(our_packet)? {
                    println!(
                        "id={:#04x}, flags={:#04x}, size={}",
                        pso_packet.packet.header.id,
                        pso_packet.packet.header.flags,
                        pso_packet.packet.header.size
                    );
                    if pso_packet.packet.body.is_empty() {
                        println!("<No data>");
                    } else {
                        println!("{:?}", pso_packet.packet.body.hex_conf(hex_cfg));
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

        println!();
    }

    Ok(())
}
