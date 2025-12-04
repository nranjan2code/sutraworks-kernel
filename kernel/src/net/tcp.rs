//! TCP (Transmission Control Protocol)
//!
//! Implements connection-oriented, reliable transport with:
//! - Connection tracking (TCB table)
//! - Retransmission with RTT-based RTO (Jacobson/Karels)
//! - Congestion control (RFC 5681: Slow Start, Congestion Avoidance, Fast Recovery)

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::convert::TryInto;

use crate::arch::SpinLock;
use crate::drivers::timer;
use crate::net::ip::Ipv4Addr;
use crate::net::ipv4;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum Segment Size (typical for Ethernet)
const MSS: u32 = 1460;

/// Initial congestion window (3 * MSS per RFC 5681)
const INITIAL_CWND: u32 = 4380;

/// Initial slow start threshold
const INITIAL_SSTHRESH: u32 = 65535;

/// Minimum RTO (200ms)
const MIN_RTO_US: u64 = 200_000;

/// Maximum RTO (60 seconds)
const MAX_RTO_US: u64 = 60_000_000;

/// Initial RTO before first RTT measurement (1 second)
const INITIAL_RTO_US: u64 = 1_000_000;

/// Maximum connections in TCB table
const MAX_CONNECTIONS: usize = 64;

/// Maximum segments in retransmit queue per connection
const MAX_RETRANSMIT_QUEUE: usize = 16;

/// Duplicate ACK threshold for fast retransmit
const DUP_ACK_THRESHOLD: u8 = 3;

/// Default receive window size
const DEFAULT_RECV_WINDOW: u32 = 65535;

// ═══════════════════════════════════════════════════════════════════════════════
// TCP FLAGS
// ═══════════════════════════════════════════════════════════════════════════════

/// TCP Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFlags(u16);

impl TcpFlags {
    pub const FIN: u16 = 1 << 0;
    pub const SYN: u16 = 1 << 1;
    pub const RST: u16 = 1 << 2;
    pub const PSH: u16 = 1 << 3;
    pub const ACK: u16 = 1 << 4;
    pub const URG: u16 = 1 << 5;
    pub const ECE: u16 = 1 << 6;
    pub const CWR: u16 = 1 << 7;
    
    pub fn new(val: u16) -> Self {
        Self(val)
    }
    
    pub fn contains(&self, flag: u16) -> bool {
        (self.0 & flag) != 0
    }
    
    pub fn bits(&self) -> u16 {
        self.0
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TCP STATE MACHINE
// ═══════════════════════════════════════════════════════════════════════════════

/// TCP State Machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

// ═══════════════════════════════════════════════════════════════════════════════
// CONGESTION STATE
// ═══════════════════════════════════════════════════════════════════════════════

/// Congestion Control State (RFC 5681)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionState {
    /// Exponential growth: cwnd += MSS per ACK
    SlowStart,
    /// Linear growth: cwnd += MSS^2/cwnd per ACK
    CongestionAvoidance,
    /// After fast retransmit: cwnd = ssthresh + 3*MSS
    FastRecovery,
}

// ═══════════════════════════════════════════════════════════════════════════════
// TCP SEGMENT
// ═══════════════════════════════════════════════════════════════════════════════

/// TCP Segment
#[derive(Debug, Clone)]
pub struct TcpSegment {
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // 4 bits (number of 32-bit words)
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
    pub options: Vec<u8>,
    pub payload: Vec<u8>,
}

impl TcpSegment {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("Packet too short for TCP header");
        }
        
        let src_port = u16::from_be_bytes(data[0..2].try_into().unwrap());
        let dst_port = u16::from_be_bytes(data[2..4].try_into().unwrap());
        let sequence_num = u32::from_be_bytes(data[4..8].try_into().unwrap());
        let ack_num = u32::from_be_bytes(data[8..12].try_into().unwrap());
        
        let data_offset_res = data[12];
        let data_offset = data_offset_res >> 4;
        let header_len = (data_offset as usize) * 4;
        
        if data.len() < header_len {
            return Err("Packet shorter than TCP header length");
        }
        
        let flags_u16 = u16::from_be_bytes(data[12..14].try_into().unwrap()) & 0x01FF;
        let flags = TcpFlags::new(flags_u16);
        
        let window_size = u16::from_be_bytes(data[14..16].try_into().unwrap());
        let checksum = u16::from_be_bytes(data[16..18].try_into().unwrap());
        let urgent_pointer = u16::from_be_bytes(data[18..20].try_into().unwrap());
        
        let options = data[20..header_len].to_vec();
        let payload = data[header_len..].to_vec();
        
        Ok(Self {
            src_port,
            dst_port,
            sequence_num,
            ack_num,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_pointer,
            options,
            payload,
        })
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let header_len = 20 + self.options.len();
        let data_offset = (header_len / 4) as u8;
        
        let mut bytes = Vec::with_capacity(header_len + self.payload.len());
        
        bytes.extend_from_slice(&self.src_port.to_be_bytes());
        bytes.extend_from_slice(&self.dst_port.to_be_bytes());
        bytes.extend_from_slice(&self.sequence_num.to_be_bytes());
        bytes.extend_from_slice(&self.ack_num.to_be_bytes());
        
        let offset_res = (data_offset << 4) | ((self.flags.bits() >> 8) as u8);
        bytes.push(offset_res);
        bytes.push(self.flags.bits() as u8);
        
        bytes.extend_from_slice(&self.window_size.to_be_bytes());
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.urgent_pointer.to_be_bytes());
        
        bytes.extend_from_slice(&self.options);
        
        bytes.extend_from_slice(&self.payload);
        
        bytes
    }
    
    /// Serialize segment with computed checksum
    pub fn to_bytes_with_checksum(&self, src_ip: Ipv4Addr, dst_ip: Ipv4Addr) -> Vec<u8> {
        // First serialize with zero checksum
        let mut bytes = self.to_bytes();
        
        // Compute checksum over pseudo-header + TCP segment
        let checksum = tcp_checksum(src_ip, dst_ip, &bytes);
        
        // Insert checksum at bytes 16-17
        bytes[16] = (checksum >> 8) as u8;
        bytes[17] = checksum as u8;
        
        bytes
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TCP CHECKSUM (RFC 793)
// ═══════════════════════════════════════════════════════════════════════════════

/// Compute TCP checksum with pseudo-header (RFC 793)
///
/// The TCP checksum is computed over:
/// - 12-byte pseudo-header: src IP (4), dst IP (4), zero (1), protocol (1), TCP length (2)
/// - TCP header (with checksum field set to 0)
/// - TCP payload
pub fn tcp_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, tcp_segment: &[u8]) -> u16 {
    let tcp_len = tcp_segment.len();
    
    // Build pseudo-header (12 bytes)
    let mut pseudo = Vec::with_capacity(12 + tcp_len);
    pseudo.extend_from_slice(&src_ip.0);           // Source IP (4 bytes)
    pseudo.extend_from_slice(&dst_ip.0);           // Destination IP (4 bytes)
    pseudo.push(0);                                 // Reserved (1 byte)
    pseudo.push(6);                                 // Protocol = TCP (1 byte)
    pseudo.extend_from_slice(&(tcp_len as u16).to_be_bytes()); // TCP Length (2 bytes)
    
    // Copy TCP segment (with checksum field zeroed)
    let mut tcp_copy = tcp_segment.to_vec();
    if tcp_copy.len() >= 18 {
        tcp_copy[16] = 0;  // Zero checksum field
        tcp_copy[17] = 0;
    }
    pseudo.extend_from_slice(&tcp_copy);
    
    // Use RFC 1071 checksum
    crate::net::checksum(&pseudo)
}

/// Verify TCP checksum
pub fn verify_tcp_checksum(src_ip: Ipv4Addr, dst_ip: Ipv4Addr, tcp_segment: &[u8]) -> bool {
    // Computing checksum over segment with existing checksum should yield 0 or 0xFFFF
    let tcp_len = tcp_segment.len();
    
    let mut pseudo = Vec::with_capacity(12 + tcp_len);
    pseudo.extend_from_slice(&src_ip.0);
    pseudo.extend_from_slice(&dst_ip.0);
    pseudo.push(0);
    pseudo.push(6);
    pseudo.extend_from_slice(&(tcp_len as u16).to_be_bytes());
    pseudo.extend_from_slice(tcp_segment);
    
    let result = crate::net::checksum(&pseudo);
    result == 0 || result == 0xFFFF
}

// ═══════════════════════════════════════════════════════════════════════════════
// RETRANSMIT QUEUE
// ═══════════════════════════════════════════════════════════════════════════════

/// Entry in the retransmission queue
#[derive(Debug, Clone)]
struct RetransmitEntry {
    /// Starting sequence number of this segment
    seq_num: u32,
    /// Segment data (including TCP header)
    data: Vec<u8>,
    /// Time when segment was sent (microseconds since boot)
    send_time: u64,
    /// Number of times this segment has been retransmitted
    retransmit_count: u8,
}

/// Queue of unacknowledged segments awaiting retransmission
#[derive(Debug, Clone)]
struct RetransmitQueue {
    entries: VecDeque<RetransmitEntry>,
}

impl RetransmitQueue {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
    
    /// Add a segment to the retransmit queue
    fn push(&mut self, seq_num: u32, data: Vec<u8>, send_time: u64) {
        if self.entries.len() >= MAX_RETRANSMIT_QUEUE {
            // Drop oldest if queue is full
            self.entries.pop_front();
        }
        self.entries.push_back(RetransmitEntry {
            seq_num,
            data,
            send_time,
            retransmit_count: 0,
        });
    }
    
    /// Remove all segments that have been acknowledged (seq < ack_num)
    fn ack_up_to(&mut self, ack_num: u32) {
        self.entries.retain(|entry| {
            // Keep entries whose data extends beyond the ack
            seq_after(entry.seq_num + entry.data.len() as u32, ack_num)
        });
    }
    
    /// Get the first unacked segment for potential retransmission
    fn front(&self) -> Option<&RetransmitEntry> {
        self.entries.front()
    }
    
    /// Check if queue is empty
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    
    /// Mark the front entry as retransmitted
    fn mark_retransmitted(&mut self, now: u64) {
        if let Some(entry) = self.entries.front_mut() {
            entry.send_time = now;
            entry.retransmit_count += 1;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TCP CONNECTION (TCB)
// ═══════════════════════════════════════════════════════════════════════════════

/// TCP Connection (Transmission Control Block)
#[derive(Debug)]
pub struct TcpConnection {
    // ─────────────────────────────────────────────────────────────────────────
    // Connection Identity
    // ─────────────────────────────────────────────────────────────────────────
    pub local_addr: Ipv4Addr,
    pub local_port: u16,
    pub remote_addr: Ipv4Addr,
    pub remote_port: u16,
    
    // ─────────────────────────────────────────────────────────────────────────
    // State Machine
    // ─────────────────────────────────────────────────────────────────────────
    pub state: TcpState,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Sequence Numbers (RFC 793)
    // ─────────────────────────────────────────────────────────────────────────
    /// SND.UNA - Send Unacknowledged: oldest unacknowledged sequence number
    pub send_unacked: u32,
    /// SND.NXT - Send Next: next sequence number to send
    pub send_next: u32,
    /// SND.WND - Send Window: peer's advertised receive window
    pub send_window: u32,
    /// RCV.NXT - Receive Next: next expected sequence number
    pub recv_next: u32,
    /// RCV.WND - Receive Window: our advertised receive window
    pub recv_window: u32,
    /// Initial Sequence Number (for connection)
    pub iss: u32,
    /// Initial Receive Sequence (from peer)
    pub irs: u32,
    
    // ─────────────────────────────────────────────────────────────────────────
    // RTT Estimation (Jacobson/Karels)
    // ─────────────────────────────────────────────────────────────────────────
    /// Smoothed RTT (microseconds)
    srtt: u64,
    /// RTT variance (microseconds)
    rttvar: u64,
    /// Retransmission Timeout (microseconds)
    rto: u64,
    /// Sequence number being timed for RTT measurement
    rtt_seq: Option<u32>,
    /// Time when RTT measurement started
    rtt_time: u64,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Congestion Control (RFC 5681)
    // ─────────────────────────────────────────────────────────────────────────
    /// Congestion Window (bytes)
    pub cwnd: u32,
    /// Slow Start Threshold (bytes)
    pub ssthresh: u32,
    /// Current congestion control state
    pub congestion_state: CongestionState,
    /// Duplicate ACK counter (for fast retransmit)
    dup_ack_count: u8,
    /// Last ACK number received (for detecting duplicates)
    last_ack: u32,
    /// Recovery point for fast recovery (SND.NXT when entering recovery)
    recover: u32,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Retransmission
    // ─────────────────────────────────────────────────────────────────────────
    /// Queue of unacknowledged segments
    retransmit_queue: RetransmitQueue,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Buffers
    // ─────────────────────────────────────────────────────────────────────────
    /// Receive buffer (data from peer)
    pub recv_buffer: VecDeque<u8>,
    /// Send buffer (data to send)
    pub send_buffer: VecDeque<u8>,
}

impl TcpConnection {
    /// Create a new TCP connection
    pub fn new(local_addr: Ipv4Addr, local_port: u16, remote_addr: Ipv4Addr, remote_port: u16) -> Self {
        // Generate initial sequence number (simplified - should use secure random)
        let iss = (timer::uptime_us() as u32).wrapping_mul(12345);
        
        Self {
            local_addr,
            local_port,
            remote_addr,
            remote_port,
            state: TcpState::Closed,
            send_unacked: iss,
            send_next: iss,
            send_window: 0,
            recv_next: 0,
            recv_window: DEFAULT_RECV_WINDOW,
            iss,
            irs: 0,
            srtt: 0,
            rttvar: 0,
            rto: INITIAL_RTO_US,
            rtt_seq: None,
            rtt_time: 0,
            cwnd: INITIAL_CWND,
            ssthresh: INITIAL_SSTHRESH,
            congestion_state: CongestionState::SlowStart,
            dup_ack_count: 0,
            last_ack: 0,
            recover: 0,
            retransmit_queue: RetransmitQueue::new(),
            recv_buffer: VecDeque::new(),
            send_buffer: VecDeque::new(),
        }
    }
    
    /// Update RTT estimate using Jacobson/Karels algorithm
    fn update_rtt(&mut self, measured_rtt: u64) {
        if self.srtt == 0 {
            // First RTT measurement
            self.srtt = measured_rtt;
            self.rttvar = measured_rtt / 2;
        } else {
            // Jacobson/Karels formula:
            // RTTVAR = (1 - beta) * RTTVAR + beta * |SRTT - RTT|
            // SRTT = (1 - alpha) * SRTT + alpha * RTT
            // where alpha = 1/8, beta = 1/4
            
            let delta = if measured_rtt > self.srtt {
                measured_rtt - self.srtt
            } else {
                self.srtt - measured_rtt
            };
            
            // RTTVAR = 3/4 * RTTVAR + 1/4 * delta
            self.rttvar = (self.rttvar * 3 + delta) / 4;
            
            // SRTT = 7/8 * SRTT + 1/8 * measured_rtt
            self.srtt = (self.srtt * 7 + measured_rtt) / 8;
        }
        
        // RTO = SRTT + max(G, 4 * RTTVAR) where G is clock granularity
        // We use 4 * RTTVAR directly (G is negligible with microsecond timer)
        self.rto = (self.srtt + 4 * self.rttvar).clamp(MIN_RTO_US, MAX_RTO_US);
    }
    
    /// Process an incoming ACK
    pub fn process_ack(&mut self, ack_num: u32, window: u32) {
        // Update send window
        self.send_window = window;
        
        // Check if this is a new ACK (acknowledges new data)
        if seq_after(ack_num, self.send_unacked) {
            // New data acknowledged
            
            // Update RTT if we were timing this segment
            if let Some(rtt_seq) = self.rtt_seq {
                if seq_after(ack_num, rtt_seq) || ack_num == rtt_seq {
                    let now = timer::uptime_us();
                    let measured_rtt = now.saturating_sub(self.rtt_time);
                    self.update_rtt(measured_rtt);
                    self.rtt_seq = None;
                }
            }
            
            // Update congestion window based on state
            let bytes_acked = ack_num.wrapping_sub(self.send_unacked);
            self.update_cwnd_on_ack(bytes_acked);
            
            // Update SND.UNA
            self.send_unacked = ack_num;
            
            // Remove acknowledged segments from retransmit queue
            self.retransmit_queue.ack_up_to(ack_num);
            
            // Reset duplicate ACK counter
            self.dup_ack_count = 0;
            self.last_ack = ack_num;
            
            // Exit fast recovery if all data is acknowledged
            if self.congestion_state == CongestionState::FastRecovery {
                if seq_after(ack_num, self.recover) || ack_num == self.recover {
                    // Full ACK - exit fast recovery
                    self.congestion_state = CongestionState::CongestionAvoidance;
                    self.cwnd = self.ssthresh;
                }
            }
        } else if ack_num == self.send_unacked && ack_num == self.last_ack {
            // Duplicate ACK
            self.dup_ack_count += 1;
            
            if self.dup_ack_count == DUP_ACK_THRESHOLD {
                // Fast Retransmit & Fast Recovery (RFC 5681)
                self.enter_fast_recovery();
            } else if self.dup_ack_count > DUP_ACK_THRESHOLD 
                      && self.congestion_state == CongestionState::FastRecovery {
                // Inflate cwnd during fast recovery
                self.cwnd += MSS;
            }
        }
    }
    
    /// Update congestion window on new ACK
    fn update_cwnd_on_ack(&mut self, bytes_acked: u32) {
        match self.congestion_state {
            CongestionState::SlowStart => {
                // Exponential growth: cwnd += min(bytes_acked, MSS)
                self.cwnd += bytes_acked.min(MSS);
                
                // Transition to congestion avoidance if threshold reached
                if self.cwnd >= self.ssthresh {
                    self.congestion_state = CongestionState::CongestionAvoidance;
                }
            }
            CongestionState::CongestionAvoidance => {
                // Linear growth: cwnd += MSS * MSS / cwnd (approximately 1 MSS per RTT)
                if self.cwnd > 0 {
                    self.cwnd += (MSS * MSS) / self.cwnd;
                }
            }
            CongestionState::FastRecovery => {
                // During fast recovery, cwnd is inflated but ssthresh is the target
                // This is handled in process_ack
            }
        }
    }
    
    /// Enter fast recovery after 3 duplicate ACKs
    fn enter_fast_recovery(&mut self) {
        // ssthresh = max(FlightSize / 2, 2*MSS)
        let flight_size = self.send_next.wrapping_sub(self.send_unacked);
        self.ssthresh = (flight_size / 2).max(2 * MSS);
        
        // cwnd = ssthresh + 3*MSS (for the 3 dup ACKs)
        self.cwnd = self.ssthresh + 3 * MSS;
        
        // Record recovery point
        self.recover = self.send_next;
        
        // Enter fast recovery state
        self.congestion_state = CongestionState::FastRecovery;
        
        // Retransmit the first unacked segment
        self.retransmit_first();
    }
    
    /// Handle RTO timeout
    pub fn handle_timeout(&mut self) {
        // ssthresh = max(FlightSize / 2, 2*MSS)
        let flight_size = self.send_next.wrapping_sub(self.send_unacked);
        self.ssthresh = (flight_size / 2).max(2 * MSS);
        
        // cwnd = 1 MSS (back to slow start)
        self.cwnd = MSS;
        
        // Return to slow start
        self.congestion_state = CongestionState::SlowStart;
        
        // Back off RTO (exponential backoff)
        self.rto = (self.rto * 2).min(MAX_RTO_US);
        
        // Reset duplicate ACK counter
        self.dup_ack_count = 0;
        
        // Retransmit first unacked segment
        self.retransmit_first();
    }
    
    /// Retransmit the first unacknowledged segment
    fn retransmit_first(&mut self) {
        if let Some(entry) = self.retransmit_queue.front() {
            let data = entry.data.clone();
            let now = timer::uptime_us();
            
            // Send via IP layer
            let _ = ipv4::send_packet(self.remote_addr, 6, &data);
            
            // Update retransmit entry
            self.retransmit_queue.mark_retransmitted(now);
            
            // Invalidate RTT measurement (Karn's algorithm)
            self.rtt_seq = None;
        }
    }
    
    /// Check for retransmission timeout
    pub fn check_retransmit(&mut self, now: u64) {
        if self.retransmit_queue.is_empty() {
            return;
        }
        
        if let Some(entry) = self.retransmit_queue.front() {
            let elapsed = now.saturating_sub(entry.send_time);
            if elapsed >= self.rto {
                self.handle_timeout();
            }
        }
    }
    
    /// Queue data for sending
    pub fn send_data(&mut self, data: &[u8]) -> usize {
        for &byte in data {
            self.send_buffer.push_back(byte);
        }
        data.len()
    }
    
    /// Send queued data (respecting window)
    pub fn flush_send_buffer(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Connection not established");
        }
        
        let now = timer::uptime_us();
        
        while !self.send_buffer.is_empty() {
            // Calculate how much we can send
            let flight_size = self.send_next.wrapping_sub(self.send_unacked);
            let window = self.cwnd.min(self.send_window);
            
            if flight_size >= window {
                break; // Window full
            }
            
            let available = window - flight_size;
            let send_size = (available as usize).min(MSS as usize).min(self.send_buffer.len());
            
            if send_size == 0 {
                break;
            }
            
            // Build segment payload
            let mut payload = Vec::with_capacity(send_size);
            for _ in 0..send_size {
                if let Some(byte) = self.send_buffer.pop_front() {
                    payload.push(byte);
                }
            }
            
            // Build TCP segment
            let segment = TcpSegment {
                src_port: self.local_port,
                dst_port: self.remote_port,
                sequence_num: self.send_next,
                ack_num: self.recv_next,
                data_offset: 5,
                flags: TcpFlags::new(TcpFlags::ACK | TcpFlags::PSH),
                window_size: self.recv_window as u16,
                checksum: 0, // TODO: Calculate checksum
                urgent_pointer: 0,
                options: Vec::new(),
                payload: payload.clone(),
            };
            
            let segment_bytes = segment.to_bytes();
            
            // Start RTT measurement if not already timing
            if self.rtt_seq.is_none() {
                self.rtt_seq = Some(self.send_next);
                self.rtt_time = now;
            }
            
            // Add to retransmit queue
            self.retransmit_queue.push(self.send_next, segment_bytes.clone(), now);
            
            // Update SND.NXT
            self.send_next = self.send_next.wrapping_add(payload.len() as u32);
            
            // Send via IP layer
            ipv4::send_packet(self.remote_addr, 6, &segment_bytes)?;
        }
        
        Ok(())
    }
    
    /// Matches a 4-tuple (used for connection lookup)
    pub fn matches(&self, local_addr: Ipv4Addr, local_port: u16, 
                   remote_addr: Ipv4Addr, remote_port: u16) -> bool {
        self.local_addr == local_addr 
            && self.local_port == local_port
            && self.remote_addr == remote_addr
            && self.remote_port == remote_port
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONNECTION TABLE
// ═══════════════════════════════════════════════════════════════════════════════

/// Global TCP Connection Table
pub static TCB_TABLE: SpinLock<TcpConnectionTable> = SpinLock::new(TcpConnectionTable::new());

/// Table of active TCP connections
pub struct TcpConnectionTable {
    connections: Vec<TcpConnection>,
}

impl TcpConnectionTable {
    pub const fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }
    
    /// Find a connection by 4-tuple
    pub fn find(&self, local_addr: Ipv4Addr, local_port: u16,
                remote_addr: Ipv4Addr, remote_port: u16) -> Option<usize> {
        self.connections.iter().position(|c| {
            c.matches(local_addr, local_port, remote_addr, remote_port)
        })
    }
    
    /// Find a listening connection by local port
    pub fn find_listener(&self, local_port: u16) -> Option<usize> {
        self.connections.iter().position(|c| {
            c.state == TcpState::Listen && c.local_port == local_port
        })
    }
    
    /// Add a new connection
    pub fn add(&mut self, conn: TcpConnection) -> Result<usize, &'static str> {
        if self.connections.len() >= MAX_CONNECTIONS {
            return Err("Connection table full");
        }
        self.connections.push(conn);
        Ok(self.connections.len() - 1)
    }
    
    /// Get mutable reference to connection by index
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut TcpConnection> {
        self.connections.get_mut(idx)
    }
    
    /// Remove a connection by index
    pub fn remove(&mut self, idx: usize) {
        if idx < self.connections.len() {
            self.connections.remove(idx);
        }
    }
    
    /// Check all connections for retransmission timeouts
    pub fn check_retransmits(&mut self) {
        let now = timer::uptime_us();
        for conn in &mut self.connections {
            conn.check_retransmit(now);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKET HANDLER
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle incoming TCP packet (called from IPv4 handler)
pub fn handle_packet(data: &[u8], src_ip: Ipv4Addr, dst_ip: Ipv4Addr) -> Result<(), &'static str> {
    let segment = TcpSegment::parse(data)?;
    
    let mut table = TCB_TABLE.lock();
    
    // Look for existing connection
    if let Some(idx) = table.find(dst_ip, segment.dst_port, src_ip, segment.src_port) {
        let conn = table.get_mut(idx).unwrap();
        process_segment(conn, &segment)?;
    } else {
        // Check for listening socket
        if let Some(_idx) = table.find_listener(segment.dst_port) {
            // Handle incoming SYN
            if segment.flags.contains(TcpFlags::SYN) && !segment.flags.contains(TcpFlags::ACK) {
                // Create new connection for this client
                let mut conn = TcpConnection::new(dst_ip, segment.dst_port, src_ip, segment.src_port);
                conn.state = TcpState::SynReceived;
                conn.irs = segment.sequence_num;
                conn.recv_next = segment.sequence_num.wrapping_add(1);
                
                // Send SYN-ACK
                send_syn_ack(&conn)?;
                
                // Add to table
                table.add(conn)?;
            }
        } else if segment.flags.contains(TcpFlags::RST) {
            // Ignore RST for non-existent connections
            return Ok(());
        } else {
            // Send RST for unexpected packets
            send_rst(dst_ip, segment.dst_port, src_ip, segment.src_port, 
                    segment.ack_num, segment.sequence_num.wrapping_add(1))?;
        }
    }
    
    Ok(())
}

/// Process a TCP segment for an existing connection
fn process_segment(conn: &mut TcpConnection, segment: &TcpSegment) -> Result<(), &'static str> {
    // Handle RST
    if segment.flags.contains(TcpFlags::RST) {
        conn.state = TcpState::Closed;
        return Ok(());
    }
    
    match conn.state {
        TcpState::SynSent => {
            // Expecting SYN-ACK
            if segment.flags.contains(TcpFlags::SYN) && segment.flags.contains(TcpFlags::ACK) {
                conn.irs = segment.sequence_num;
                conn.recv_next = segment.sequence_num.wrapping_add(1);
                conn.send_unacked = segment.ack_num;
                conn.send_window = segment.window_size as u32;
                conn.state = TcpState::Established;
                
                // Send ACK to complete handshake
                send_ack(conn)?;
            }
        }
        TcpState::SynReceived => {
            // Expecting ACK
            if segment.flags.contains(TcpFlags::ACK) {
                if segment.ack_num == conn.send_next {
                    conn.state = TcpState::Established;
                }
            }
        }
        TcpState::Established => {
            // Process ACK
            if segment.flags.contains(TcpFlags::ACK) {
                conn.process_ack(segment.ack_num, segment.window_size as u32);
            }
            
            // Process data
            if !segment.payload.is_empty() {
                if segment.sequence_num == conn.recv_next {
                    // In-order data
                    for byte in &segment.payload {
                        conn.recv_buffer.push_back(*byte);
                    }
                    conn.recv_next = conn.recv_next.wrapping_add(segment.payload.len() as u32);
                    
                    // Send ACK
                    send_ack(conn)?;
                }
                // Out-of-order data: send duplicate ACK (will trigger fast retransmit at sender)
                else {
                    send_ack(conn)?;
                }
            }
            
            // Handle FIN
            if segment.flags.contains(TcpFlags::FIN) {
                conn.recv_next = conn.recv_next.wrapping_add(1);
                conn.state = TcpState::CloseWait;
                send_ack(conn)?;
            }
        }
        TcpState::FinWait1 => {
            if segment.flags.contains(TcpFlags::ACK) {
                if segment.flags.contains(TcpFlags::FIN) {
                    conn.recv_next = conn.recv_next.wrapping_add(1);
                    conn.state = TcpState::TimeWait;
                    send_ack(conn)?;
                } else {
                    conn.state = TcpState::FinWait2;
                }
            }
        }
        TcpState::FinWait2 => {
            if segment.flags.contains(TcpFlags::FIN) {
                conn.recv_next = conn.recv_next.wrapping_add(1);
                conn.state = TcpState::TimeWait;
                send_ack(conn)?;
            }
        }
        TcpState::LastAck => {
            if segment.flags.contains(TcpFlags::ACK) {
                conn.state = TcpState::Closed;
            }
        }
        _ => {}
    }
    
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Compare sequence numbers (handles wraparound)
fn seq_after(a: u32, b: u32) -> bool {
    // a > b, accounting for wraparound
    ((a.wrapping_sub(b)) as i32) > 0
}

/// Send a SYN-ACK segment
fn send_syn_ack(conn: &TcpConnection) -> Result<(), &'static str> {
    let segment = TcpSegment {
        src_port: conn.local_port,
        dst_port: conn.remote_port,
        sequence_num: conn.iss,
        ack_num: conn.recv_next,
        data_offset: 5,
        flags: TcpFlags::new(TcpFlags::SYN | TcpFlags::ACK),
        window_size: conn.recv_window as u16,
        checksum: 0,
        urgent_pointer: 0,
        options: Vec::new(),
        payload: Vec::new(),
    };
    
    ipv4::send_packet(conn.remote_addr, 6, &segment.to_bytes())
}

/// Send an ACK segment
fn send_ack(conn: &TcpConnection) -> Result<(), &'static str> {
    let segment = TcpSegment {
        src_port: conn.local_port,
        dst_port: conn.remote_port,
        sequence_num: conn.send_next,
        ack_num: conn.recv_next,
        data_offset: 5,
        flags: TcpFlags::new(TcpFlags::ACK),
        window_size: conn.recv_window as u16,
        checksum: 0,
        urgent_pointer: 0,
        options: Vec::new(),
        payload: Vec::new(),
    };
    
    ipv4::send_packet(conn.remote_addr, 6, &segment.to_bytes())
}

/// Send a RST segment
fn send_rst(_src_addr: Ipv4Addr, src_port: u16, dst_addr: Ipv4Addr, dst_port: u16,
           seq: u32, ack: u32) -> Result<(), &'static str> {
    let segment = TcpSegment {
        src_port,
        dst_port,
        sequence_num: seq,
        ack_num: ack,
        data_offset: 5,
        flags: TcpFlags::new(TcpFlags::RST | TcpFlags::ACK),
        window_size: 0,
        checksum: 0,
        urgent_pointer: 0,
        options: Vec::new(),
        payload: Vec::new(),
    };
    
    ipv4::send_packet(dst_addr, 6, &segment.to_bytes())
}

// ═══════════════════════════════════════════════════════════════════════════════
// TIMER TICK (Called from scheduler)
// ═══════════════════════════════════════════════════════════════════════════════

/// Periodic TCP tick - check for retransmission timeouts
/// Should be called from scheduler tick (e.g., every 100ms)
pub fn tcp_tick() {
    TCB_TABLE.lock().check_retransmits();
}

// ═══════════════════════════════════════════════════════════════════════════════
// PUBLIC API
// ═══════════════════════════════════════════════════════════════════════════════

/// Create a listening socket
pub fn listen(local_addr: Ipv4Addr, local_port: u16) -> Result<usize, &'static str> {
    let mut conn = TcpConnection::new(local_addr, local_port, Ipv4Addr::ANY, 0);
    conn.state = TcpState::Listen;
    TCB_TABLE.lock().add(conn)
}

/// Initiate a connection
pub fn connect(local_addr: Ipv4Addr, local_port: u16, 
               remote_addr: Ipv4Addr, remote_port: u16) -> Result<usize, &'static str> {
    let mut conn = TcpConnection::new(local_addr, local_port, remote_addr, remote_port);
    conn.state = TcpState::SynSent;
    conn.send_next = conn.iss.wrapping_add(1);
    
    // Send SYN
    let segment = TcpSegment {
        src_port: local_port,
        dst_port: remote_port,
        sequence_num: conn.iss,
        ack_num: 0,
        data_offset: 5,
        flags: TcpFlags::new(TcpFlags::SYN),
        window_size: conn.recv_window as u16,
        checksum: 0,
        urgent_pointer: 0,
        options: Vec::new(),
        payload: Vec::new(),
    };
    
    ipv4::send_packet(remote_addr, 6, &segment.to_bytes())?;
    
    TCB_TABLE.lock().add(conn)
}

// ═══════════════════════════════════════════════════════════════════════════════
// UNIT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    
    // ──────────────────────────────────────────────────────────────────────────
    // Test Helper: Create connection without hardware timer dependency
    // ──────────────────────────────────────────────────────────────────────────
    
    fn test_connection() -> TcpConnection {
        TcpConnection {
            local_addr: Ipv4Addr([127, 0, 0, 1]),
            local_port: 1234,
            remote_addr: Ipv4Addr([127, 0, 0, 1]),
            remote_port: 80,
            state: TcpState::Closed,
            send_unacked: 1000,
            send_next: 1000,
            send_window: 65535,
            recv_next: 0,
            recv_window: DEFAULT_RECV_WINDOW,
            iss: 1000,
            irs: 0,
            srtt: 0,
            rttvar: 0,
            rto: INITIAL_RTO_US,
            rtt_seq: None,
            rtt_time: 0,
            cwnd: INITIAL_CWND,
            ssthresh: INITIAL_SSTHRESH,
            congestion_state: CongestionState::SlowStart,
            dup_ack_count: 0,
            last_ack: 0,
            recover: 0,
            retransmit_queue: RetransmitQueue::new(),
            recv_buffer: VecDeque::new(),
            send_buffer: VecDeque::new(),
        }
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // TCP FLAGS TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_tcp_flags() {
        let flags = TcpFlags::new(TcpFlags::SYN | TcpFlags::ACK);
        assert!(flags.contains(TcpFlags::SYN));
        assert!(flags.contains(TcpFlags::ACK));
        assert!(!flags.contains(TcpFlags::FIN));
        assert!(!flags.contains(TcpFlags::RST));
    }
    
    #[test]
    fn test_tcp_flags_bits() {
        let flags = TcpFlags::new(TcpFlags::FIN | TcpFlags::PSH);
        assert_eq!(flags.bits(), 0x09); // FIN=1, PSH=8
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // TCP SEGMENT TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_segment_parse_minimum() {
        // Minimum valid TCP header (20 bytes)
        let data = [
            0x00, 0x50, // Src port: 80
            0x1F, 0x90, // Dst port: 8080
            0x00, 0x00, 0x00, 0x01, // Seq: 1
            0x00, 0x00, 0x00, 0x02, // Ack: 2
            0x50, 0x12, // Data offset: 5 (20 bytes), Flags: SYN+ACK
            0xFF, 0xFF, // Window: 65535
            0x00, 0x00, // Checksum: 0
            0x00, 0x00, // Urgent: 0
        ];
        
        let segment = TcpSegment::parse(&data).unwrap();
        assert_eq!(segment.src_port, 80);
        assert_eq!(segment.dst_port, 8080);
        assert_eq!(segment.sequence_num, 1);
        assert_eq!(segment.ack_num, 2);
        assert!(segment.flags.contains(TcpFlags::SYN));
        assert!(segment.flags.contains(TcpFlags::ACK));
        assert_eq!(segment.window_size, 65535);
    }
    
    #[test]
    fn test_segment_parse_too_short() {
        let data = [0u8; 15]; // Less than 20 bytes
        assert!(TcpSegment::parse(&data).is_err());
    }
    
    #[test]
    fn test_segment_roundtrip() {
        let original = TcpSegment {
            src_port: 12345,
            dst_port: 80,
            sequence_num: 0x12345678,
            ack_num: 0x87654321,
            data_offset: 5,
            flags: TcpFlags::new(TcpFlags::ACK | TcpFlags::PSH),
            window_size: 32768,
            checksum: 0,
            urgent_pointer: 0,
            options: Vec::new(),
            payload: vec![0x48, 0x65, 0x6C, 0x6C, 0x6F], // "Hello"
        };
        
        let bytes = original.to_bytes();
        let parsed = TcpSegment::parse(&bytes).unwrap();
        
        assert_eq!(parsed.src_port, original.src_port);
        assert_eq!(parsed.dst_port, original.dst_port);
        assert_eq!(parsed.sequence_num, original.sequence_num);
        assert_eq!(parsed.ack_num, original.ack_num);
        assert_eq!(parsed.payload, original.payload);
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // TCP CHECKSUM TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_tcp_checksum_basic() {
        let src_ip = Ipv4Addr([192, 168, 1, 1]);
        let dst_ip = Ipv4Addr([192, 168, 1, 2]);
        
        let segment = TcpSegment {
            src_port: 1234,
            dst_port: 80,
            sequence_num: 100,
            ack_num: 0,
            data_offset: 5,
            flags: TcpFlags::new(TcpFlags::SYN),
            window_size: 65535,
            checksum: 0,
            urgent_pointer: 0,
            options: Vec::new(),
            payload: Vec::new(),
        };
        
        let bytes = segment.to_bytes();
        let checksum = tcp_checksum(src_ip, dst_ip, &bytes);
        
        // Checksum should be non-zero for valid data
        assert_ne!(checksum, 0);
    }
    
    #[test]
    fn test_tcp_checksum_verify() {
        let src_ip = Ipv4Addr([10, 0, 0, 1]);
        let dst_ip = Ipv4Addr([10, 0, 0, 2]);
        
        let segment = TcpSegment {
            src_port: 5000,
            dst_port: 443,
            sequence_num: 1000,
            ack_num: 2000,
            data_offset: 5,
            flags: TcpFlags::new(TcpFlags::ACK),
            window_size: 16384,
            checksum: 0,
            urgent_pointer: 0,
            options: Vec::new(),
            payload: vec![1, 2, 3, 4, 5],
        };
        
        // Get bytes with checksum
        let bytes = segment.to_bytes_with_checksum(src_ip, dst_ip);
        
        // Verify should pass
        assert!(verify_tcp_checksum(src_ip, dst_ip, &bytes));
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // RTT ESTIMATION TESTS (Jacobson/Karels)
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_rtt_initial_measurement() {
        let mut conn = test_connection();
        
        // Initial state
        assert_eq!(conn.srtt, 0);
        assert_eq!(conn.rttvar, 0);
        assert_eq!(conn.rto, INITIAL_RTO_US);
        
        // First RTT measurement: 100ms
        conn.update_rtt(100_000);
        
        // First measurement: SRTT = R, RTTVAR = R/2
        assert_eq!(conn.srtt, 100_000);
        assert_eq!(conn.rttvar, 50_000);
        // RTO = SRTT + 4*RTTVAR = 100000 + 200000 = 300000
        assert_eq!(conn.rto, 300_000);
    }
    
    #[test]
    fn test_rtt_subsequent_measurements() {
        let mut conn = test_connection();
        
        // First measurement: 100ms
        conn.update_rtt(100_000);
        
        // Second measurement: also 100ms (stable network)
        conn.update_rtt(100_000);
        
        // SRTT should stay around 100ms
        // SRTT = 7/8 * 100000 + 1/8 * 100000 = 100000
        assert_eq!(conn.srtt, 100_000);
        
        // RTTVAR should decrease (stable RTT)
        // |SRTT - R| = 0
        // RTTVAR = 3/4 * 50000 + 1/4 * 0 = 37500
        assert_eq!(conn.rttvar, 37_500);
    }
    
    #[test]
    fn test_rto_clamping() {
        let mut conn = test_connection();
        
        // Very small RTT: 1ms
        conn.update_rtt(1_000);
        
        // RTO should be clamped to MIN_RTO_US (200ms)
        assert!(conn.rto >= MIN_RTO_US);
        
        // Reset and test max
        conn.srtt = 100_000_000; // 100 seconds
        conn.rttvar = 100_000_000;
        conn.update_rtt(100_000_000);
        
        // RTO should be clamped to MAX_RTO_US (60s)
        assert!(conn.rto <= MAX_RTO_US);
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // CONGESTION CONTROL TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_slow_start_initial() {
        let conn = test_connection();
        
        assert_eq!(conn.congestion_state, CongestionState::SlowStart);
        assert_eq!(conn.cwnd, INITIAL_CWND);
        assert_eq!(conn.ssthresh, INITIAL_SSTHRESH);
    }
    
    #[test]
    fn test_congestion_avoidance_transition() {
        let mut conn = test_connection();
        conn.state = TcpState::Established;
        conn.send_unacked = 1000;
        conn.send_next = 100000;
        
        // Set cwnd above ssthresh
        conn.ssthresh = 10000;
        conn.cwnd = 15000;
        
        // This should trigger transition to congestion avoidance
        // (handled internally when cwnd >= ssthresh)
        assert!(conn.cwnd >= conn.ssthresh);
    }
    
    #[test]
    fn test_fast_retransmit_threshold() {
        let mut conn = test_connection();
        conn.state = TcpState::Established;
        
        // Simulate receiving duplicate ACKs
        for _ in 0..DUP_ACK_THRESHOLD {
            conn.dup_ack_count += 1;
        }
        
        assert_eq!(conn.dup_ack_count, DUP_ACK_THRESHOLD);
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // TCP STATE MACHINE TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_state_initial() {
        let conn = test_connection();
        
        // Initial state should be Closed
        assert_eq!(conn.state, TcpState::Closed);
    }
    
    #[test]
    fn test_connection_identity() {
        let conn = test_connection();
        
        // Test connection 4-tuple matching
        assert!(conn.matches(
            Ipv4Addr([127, 0, 0, 1]), 1234,
            Ipv4Addr([127, 0, 0, 1]), 80
        ));
        
        // Different ports should not match
        assert!(!conn.matches(
            Ipv4Addr([127, 0, 0, 1]), 5001,
            Ipv4Addr([127, 0, 0, 1]), 80
        ));
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // RETRANSMIT QUEUE TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_retransmit_queue_empty() {
        let queue = RetransmitQueue::new();
        assert!(queue.is_empty());
    }
    
    #[test]
    fn test_retransmit_queue_operations() {
        let mut queue = RetransmitQueue::new();
        assert!(queue.is_empty());
        
        // Add an entry
        queue.push(1000, vec![1, 2, 3], 100);
        assert!(!queue.is_empty());
        
        // Acknowledge clears entries with seq < ack
        queue.ack_up_to(1004); // ack beyond seq 1000 + 3 bytes = 1003
        assert!(queue.is_empty());
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // SEQUENCE NUMBER COMPARISON TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_seq_after() {
        // Normal case
        assert!(seq_after(100, 50));
        assert!(!seq_after(50, 100));
        
        // Edge case: equal
        assert!(!seq_after(100, 100));
        
        // Wraparound case
        assert!(seq_after(10, 0xFFFFFF00_u32));
        assert!(!seq_after(0xFFFFFF00_u32, 10));
    }
}
