//! ICMP (Internet Control Message Protocol)
//!
//! Handles ping (echo request/reply) and error messages.

use super::{Ipv4Addr, checksum};
use super::ipv4;

/// Handle incoming ICMP packet
pub fn handle_packet(data: &[u8], src_ip: Ipv4Addr) -> Result<(), &'static str> {
    if data.len() < 8 {
        return Err("ICMP packet too short");
    }

    let icmp_type = data[0];
    let icmp_code = data[1];
    let checksum_received = u16::from_be_bytes([data[2], data[3]]);

    // Verify checksum
    let mut data_copy = [0u8; 1500];
    let len = data.len().min(1500);
    data_copy[..len].copy_from_slice(&data[..len]);
    data_copy[2] = 0;  // Clear checksum
    data_copy[3] = 0;

    let checksum_calculated = checksum(&data_copy[..len]);
    if checksum_received != checksum_calculated {
        return Err("ICMP checksum mismatch");
    }

    match icmp_type {
        8 => {
            // Echo Request (Ping) - Send Echo Reply
            send_echo_reply(src_ip, &data[4..])?;
            crate::kprintln!("[ICMP] Ping from {}.{}.{}.{}",
                src_ip.0[0], src_ip.0[1], src_ip.0[2], src_ip.0[3]);
        }
        0 => {
            // Echo Reply
            crate::kprintln!("[ICMP] Pong from {}.{}.{}.{}",
                src_ip.0[0], src_ip.0[1], src_ip.0[2], src_ip.0[3]);
        }
        _ => {
            // Other ICMP types (ignore for now)
        }
    }

    Ok(())
}

/// Send ICMP Echo Reply
fn send_echo_reply(dst_ip: Ipv4Addr, echo_data: &[u8]) -> Result<(), &'static str> {
    let mut icmp_packet = [0u8; 1480];

    if echo_data.len() > 1472 {  // Max payload
        return Err("Echo data too large");
    }

    // ICMP header
    icmp_packet[0] = 0;  // Type: Echo Reply
    icmp_packet[1] = 0;  // Code
    icmp_packet[2..4].copy_from_slice(&0u16.to_be_bytes());  // Checksum (calculated later)

    // Copy echo data (ID, Sequence, Data)
    icmp_packet[4..4 + echo_data.len()].copy_from_slice(echo_data);

    // Calculate checksum
    let packet_len = 4 + echo_data.len();
    let icmp_checksum = checksum(&icmp_packet[..packet_len]);
    icmp_packet[2..4].copy_from_slice(&icmp_checksum.to_be_bytes());

    // Send via IP layer
    ipv4::send_packet(dst_ip, 1, &icmp_packet[..packet_len])
}

/// Send ICMP Echo Request (Ping)
pub fn send_ping(dst_ip: Ipv4Addr, sequence: u16) -> Result<(), &'static str> {
    let mut icmp_packet = [0u8; 64];

    // ICMP header
    icmp_packet[0] = 8;  // Type: Echo Request
    icmp_packet[1] = 0;  // Code
    icmp_packet[2..4].copy_from_slice(&0u16.to_be_bytes());  // Checksum

    // Identifier (arbitrary)
    icmp_packet[4..6].copy_from_slice(&0x1234u16.to_be_bytes());

    // Sequence number
    icmp_packet[6..8].copy_from_slice(&sequence.to_be_bytes());

    // Payload (timestamp or pattern)
    for i in 0..56 {
        icmp_packet[8 + i] = (i as u8);
    }

    // Calculate checksum
    let icmp_checksum = checksum(&icmp_packet);
    icmp_packet[2..4].copy_from_slice(&icmp_checksum.to_be_bytes());

    // Send via IP layer
    ipv4::send_packet(dst_ip, 1, &icmp_packet)
}
