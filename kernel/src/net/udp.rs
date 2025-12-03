//! UDP (User Datagram Protocol)
//!
//! Connectionless transport layer protocol.

use super::{Ipv4Addr, checksum};
use super::ipv4;

/// Handle incoming UDP packet
pub fn handle_packet(data: &[u8], src_ip: Ipv4Addr) -> Result<(), &'static str> {
    if data.len() < 8 {
        return Err("UDP packet too short");
    }

    let src_port = u16::from_be_bytes([data[0], data[1]]);
    let dst_port = u16::from_be_bytes([data[2], data[3]]);
    let length = u16::from_be_bytes([data[4], data[5]]);
    let checksum_received = u16::from_be_bytes([data[6], data[7]]);

    let payload = &data[8..length as usize];

    crate::kprintln!("[UDP] Received from {}.{}.{}.{}:{} (len={})",
        src_ip.0[0], src_ip.0[1], src_ip.0[2], src_ip.0[3], src_port, payload.len());

    // TODO: Dispatch to socket handlers based on dst_port

    Ok(())
}

/// Send UDP packet
pub fn send_packet(dst_ip: Ipv4Addr, src_port: u16, dst_port: u16, payload: &[u8]) -> Result<(), &'static str> {
    if payload.len() > 1472 {  // Max UDP payload
        return Err("Payload too large");
    }

    let mut udp_packet = [0u8; 1480];
    let total_len = 8 + payload.len();

    // UDP header
    udp_packet[0..2].copy_from_slice(&src_port.to_be_bytes());
    udp_packet[2..4].copy_from_slice(&dst_port.to_be_bytes());
    udp_packet[4..6].copy_from_slice(&(total_len as u16).to_be_bytes());
    udp_packet[6..8].copy_from_slice(&0u16.to_be_bytes());  // Checksum (optional in IPv4)

    // Payload
    udp_packet[8..8 + payload.len()].copy_from_slice(payload);

    // Send via IP layer (protocol 17 = UDP)
    ipv4::send_packet(dst_ip, 17, &udp_packet[..total_len])
}
