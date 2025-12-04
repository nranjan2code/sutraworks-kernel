//! IPv4 (Internet Protocol version 4)
//!
//! Handles IP packet routing and forwarding.

use super::{config, checksum};
use super::ip::Ipv4Addr;
use super::arp;
use super::icmp;

/// Handle incoming IPv4 packet
pub fn handle_packet(data: &[u8]) -> Result<(), &'static str> {
    if data.len() < 20 {
        return Err("IP packet too short");
    }

    // Parse IP header
    let version = data[0] >> 4;
    let ihl = (data[0] & 0x0F) as usize;
    let header_len = ihl * 4;

    if version != 4 {
        return Err("Not IPv4");
    }

    if data.len() < header_len {
        return Err("IP header truncated");
    }

    let total_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    let protocol = data[9];
    let src_ip = Ipv4Addr([data[12], data[13], data[14], data[15]]);
    let dst_ip = Ipv4Addr([data[16], data[17], data[18], data[19]]);

    // Check if packet is for us
    let our_ip = config().ip_addr;
    if dst_ip != our_ip && dst_ip != Ipv4Addr::BROADCAST {
        return Ok(());  // Not for us, ignore
    }

    // Verify checksum
    let received_checksum = u16::from_be_bytes([data[10], data[11]]);
    let mut header_copy = [0u8; 60];  // Max IP header size
    header_copy[..header_len].copy_from_slice(&data[..header_len]);
    header_copy[10] = 0;  // Clear checksum field
    header_copy[11] = 0;

    let calculated_checksum = checksum(&header_copy[..header_len]);
    if received_checksum != calculated_checksum {
        return Err("IP checksum mismatch");
    }

    // Extract payload
    if data.len() < total_len || total_len < header_len {
        return Err("IP packet length mismatch");
    }

    let payload = &data[header_len..total_len];

    // Dispatch based on protocol
    match protocol {
        1 => icmp::handle_packet(payload, src_ip),  // ICMP
        17 => super::udp::handle_packet(payload, src_ip),  // UDP
        6 => super::tcp::handle_packet(payload, src_ip, dst_ip),  // TCP
        _ => Ok(()),  // Unknown protocol
    }
}

/// Send an IPv4 packet
pub fn send_packet(dst_ip: Ipv4Addr, protocol: u8, payload: &[u8]) -> Result<(), &'static str> {
    if payload.len() > 1480 {  // Max payload (1500 MTU - 20 IP header)
        return Err("Payload too large");
    }

    let cfg = config();

    // Resolve destination MAC address
    let dst_mac = if cfg.is_local(dst_ip) {
        // Local network - ARP for target
        arp::resolve(dst_ip).ok_or("ARP resolution failed")?
    } else {
        // Remote network - ARP for gateway
        arp::resolve(cfg.gateway).ok_or("Gateway ARP failed")?
    };

    // Build IP packet
    let total_len = 20 + payload.len();
    let mut packet = [0u8; 1518];  // Max Ethernet frame

    // Ethernet header
    packet[0..6].copy_from_slice(&dst_mac.0);
    packet[6..12].copy_from_slice(&cfg.mac_addr.0);
    packet[12..14].copy_from_slice(&0x0800u16.to_be_bytes());  // EtherType: IPv4

    // IP header
    packet[14] = 0x45;  // Version 4, IHL 5 (20 bytes)
    packet[15] = 0;     // DSCP/ECN
    packet[16..18].copy_from_slice(&(total_len as u16).to_be_bytes());  // Total Length
    packet[18..20].copy_from_slice(&0u16.to_be_bytes());  // Identification
    packet[20..22].copy_from_slice(&0u16.to_be_bytes());  // Flags/Fragment Offset
    packet[22] = 64;    // TTL
    packet[23] = protocol;
    packet[24..26].copy_from_slice(&0u16.to_be_bytes());  // Checksum (calculated later)
    packet[26..30].copy_from_slice(&cfg.ip_addr.0);  // Source IP
    packet[30..34].copy_from_slice(&dst_ip.0);       // Dest IP

    // Calculate IP checksum
    let ip_checksum = checksum(&packet[14..34]);
    packet[24..26].copy_from_slice(&ip_checksum.to_be_bytes());

    // Copy payload
    packet[34..34 + payload.len()].copy_from_slice(payload);

    // Send Ethernet frame
    super::interface::send_frame(&packet[..14 + total_len])
}
