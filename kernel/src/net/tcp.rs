//! TCP (Transmission Control Protocol)
//!
//! Connection-oriented, reliable transport layer protocol.
//! This is a simplified implementation for embedded systems.

use super::{Ipv4Addr, checksum, config};
use super::ipv4;
use crate::arch::SpinLock;
use alloc::vec::Vec;

/// TCP Connection State
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// TCP Connection Control Block
pub struct TcpSocket {
    pub local_port: u16,
    pub remote_ip: Ipv4Addr,
    pub remote_port: u16,
    pub state: TcpState,
    pub seq_num: u32,
    pub ack_num: u32,
    pub recv_buffer: Vec<u8>,
}

impl TcpSocket {
    pub fn new(local_port: u16) -> Self {
        Self {
            local_port,
            remote_ip: Ipv4Addr::UNSPECIFIED,
            remote_port: 0,
            state: TcpState::Closed,
            seq_num: 0,
            ack_num: 0,
            recv_buffer: Vec::new(),
        }
    }
}

static TCP_SOCKETS: SpinLock<Vec<TcpSocket>> = SpinLock::new(Vec::new());

/// Handle incoming TCP packet
pub fn handle_packet(data: &[u8], src_ip: Ipv4Addr, dst_ip: Ipv4Addr) -> Result<(), &'static str> {
    if data.len() < 20 {
        return Err("TCP packet too short");
    }

    let src_port = u16::from_be_bytes([data[0], data[1]]);
    let dst_port = u16::from_be_bytes([data[2], data[3]]);
    let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let ack_num = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let data_offset = (data[12] >> 4) as usize;
    let flags = data[13];
    let window = u16::from_be_bytes([data[14], data[15]]);

    let header_len = data_offset * 4;
    if data.len() < header_len {
        return Err("TCP header truncated");
    }

    let payload = &data[header_len..];

    crate::kprintln!("[TCP] {}.{}.{}.{}:{} -> :{} (flags={:#x}, seq={}, ack={}, len={})",
        src_ip.0[0], src_ip.0[1], src_ip.0[2], src_ip.0[3], src_port,
        dst_port, flags, seq_num, ack_num, payload.len());

    // Find matching socket
    let mut sockets = TCP_SOCKETS.lock();
    let socket = sockets.iter_mut().find(|s| s.local_port == dst_port);

    if let Some(socket) = socket {
        handle_tcp_state(socket, src_ip, src_port, seq_num, ack_num, flags, payload)?;
    } else {
        // No socket listening - send RST
        send_tcp_rst(dst_ip, src_ip, dst_port, src_port, ack_num)?;
    }

    Ok(())
}

/// Handle TCP state machine
fn handle_tcp_state(
    socket: &mut TcpSocket,
    remote_ip: Ipv4Addr,
    remote_port: u16,
    seq_num: u32,
    ack_num: u32,
    flags: u8,
    payload: &[u8],
) -> Result<(), &'static str> {
    let syn = (flags & 0x02) != 0;
    let ack = (flags & 0x10) != 0;
    let fin = (flags & 0x01) != 0;
    let rst = (flags & 0x04) != 0;

    match socket.state {
        TcpState::Listen => {
            if syn && !ack {
                // SYN received - send SYN-ACK
                socket.remote_ip = remote_ip;
                socket.remote_port = remote_port;
                socket.ack_num = seq_num.wrapping_add(1);
                socket.seq_num = 1000;  // Initial sequence number

                send_tcp_syn_ack(socket)?;
                socket.state = TcpState::SynReceived;
            }
        }
        TcpState::SynReceived => {
            if ack {
                // ACK received - connection established
                socket.state = TcpState::Established;
                crate::kprintln!("[TCP] Connection established");
            }
        }
        TcpState::Established => {
            if fin {
                // FIN received - close connection
                socket.ack_num = seq_num.wrapping_add(1);
                send_tcp_ack(socket)?;
                socket.state = TcpState::CloseWait;
            } else if payload.len() > 0 {
                // Data received - store in buffer and ACK
                socket.recv_buffer.extend_from_slice(payload);
                socket.ack_num = seq_num.wrapping_add(payload.len() as u32);
                send_tcp_ack(socket)?;
            }
        }
        _ => {
            // Simplified state handling
        }
    }

    Ok(())
}

/// Send TCP SYN-ACK
fn send_tcp_syn_ack(socket: &TcpSocket) -> Result<(), &'static str> {
    send_tcp_packet(socket, 0x12, &[])  // SYN + ACK
}

/// Send TCP ACK
fn send_tcp_ack(socket: &TcpSocket) -> Result<(), &'static str> {
    send_tcp_packet(socket, 0x10, &[])  // ACK
}

/// Send TCP RST
fn send_tcp_rst(
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    seq_num: u32,
) -> Result<(), &'static str> {
    let mut tcp_packet = [0u8; 20];

    tcp_packet[0..2].copy_from_slice(&src_port.to_be_bytes());
    tcp_packet[2..4].copy_from_slice(&dst_port.to_be_bytes());
    tcp_packet[4..8].copy_from_slice(&seq_num.to_be_bytes());
    tcp_packet[8..12].copy_from_slice(&0u32.to_be_bytes());
    tcp_packet[12] = 5 << 4;  // Data offset
    tcp_packet[13] = 0x04;    // RST flag
    tcp_packet[14..16].copy_from_slice(&0u16.to_be_bytes());  // Window
    tcp_packet[16..18].copy_from_slice(&0u16.to_be_bytes());  // Checksum
    tcp_packet[18..20].copy_from_slice(&0u16.to_be_bytes());  // Urgent pointer

    // Calculate checksum with pseudo-header
    let tcp_checksum = calculate_tcp_checksum(&src_ip, &dst_ip, &tcp_packet);
    tcp_packet[16..18].copy_from_slice(&tcp_checksum.to_be_bytes());

    ipv4::send_packet(dst_ip, 6, &tcp_packet)
}

/// Send TCP packet
fn send_tcp_packet(socket: &TcpSocket, flags: u8, payload: &[u8]) -> Result<(), &'static str> {
    let mut tcp_packet = [0u8; 1480];
    let header_len = 20;
    let total_len = header_len + payload.len();

    // TCP header
    tcp_packet[0..2].copy_from_slice(&socket.local_port.to_be_bytes());
    tcp_packet[2..4].copy_from_slice(&socket.remote_port.to_be_bytes());
    tcp_packet[4..8].copy_from_slice(&socket.seq_num.to_be_bytes());
    tcp_packet[8..12].copy_from_slice(&socket.ack_num.to_be_bytes());
    tcp_packet[12] = 5 << 4;  // Data offset (5 * 4 = 20 bytes)
    tcp_packet[13] = flags;
    tcp_packet[14..16].copy_from_slice(&8192u16.to_be_bytes());  // Window size
    tcp_packet[16..18].copy_from_slice(&0u16.to_be_bytes());     // Checksum
    tcp_packet[18..20].copy_from_slice(&0u16.to_be_bytes());     // Urgent pointer

    // Payload
    tcp_packet[header_len..total_len].copy_from_slice(payload);

    // Calculate checksum
    let our_ip = config().ip_addr;
    let tcp_checksum = calculate_tcp_checksum(&our_ip, &socket.remote_ip, &tcp_packet[..total_len]);
    tcp_packet[16..18].copy_from_slice(&tcp_checksum.to_be_bytes());

    ipv4::send_packet(socket.remote_ip, 6, &tcp_packet[..total_len])
}

/// Calculate TCP checksum with pseudo-header
fn calculate_tcp_checksum(src_ip: &Ipv4Addr, dst_ip: &Ipv4Addr, tcp_data: &[u8]) -> u16 {
    let mut pseudo_header = [0u8; 12];

    pseudo_header[0..4].copy_from_slice(&src_ip.0);
    pseudo_header[4..8].copy_from_slice(&dst_ip.0);
    pseudo_header[8] = 0;
    pseudo_header[9] = 6;  // Protocol: TCP
    pseudo_header[10..12].copy_from_slice(&(tcp_data.len() as u16).to_be_bytes());

    // Combine pseudo-header and TCP data
    let mut combined = Vec::new();
    combined.extend_from_slice(&pseudo_header);
    combined.extend_from_slice(tcp_data);

    checksum(&combined)
}

/// Create a listening TCP socket
pub fn listen(port: u16) -> Result<(), &'static str> {
    let mut sockets = TCP_SOCKETS.lock();

    // Check if port already in use
    if sockets.iter().any(|s| s.local_port == port) {
        return Err("Port already in use");
    }

    let mut socket = TcpSocket::new(port);
    socket.state = TcpState::Listen;
    sockets.push(socket);

    crate::kprintln!("[TCP] Listening on port {}", port);
    Ok(())
}
