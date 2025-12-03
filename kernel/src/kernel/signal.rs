//! Signal Definitions
//!
//! Based on Linux signal numbers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Signal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
}

impl Signal {
    pub fn from_i32(n: i32) -> Option<Self> {
        match n {
            1 => Some(Signal::SIGHUP),
            2 => Some(Signal::SIGINT),
            3 => Some(Signal::SIGQUIT),
            4 => Some(Signal::SIGILL),
            5 => Some(Signal::SIGTRAP),
            6 => Some(Signal::SIGABRT),
            7 => Some(Signal::SIGBUS),
            8 => Some(Signal::SIGFPE),
            9 => Some(Signal::SIGKILL),
            10 => Some(Signal::SIGUSR1),
            11 => Some(Signal::SIGSEGV),
            12 => Some(Signal::SIGUSR2),
            13 => Some(Signal::SIGPIPE),
            14 => Some(Signal::SIGALRM),
            15 => Some(Signal::SIGTERM),
            17 => Some(Signal::SIGCHLD),
            18 => Some(Signal::SIGCONT),
            19 => Some(Signal::SIGSTOP),
            20 => Some(Signal::SIGTSTP),
            21 => Some(Signal::SIGTTIN),
            22 => Some(Signal::SIGTTOU),
            _ => None,
        }
    }
}

/// Signal Action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SigAction {
    pub handler: u64, // Function pointer or SIG_DFL (0) / SIG_IGN (1)
    pub flags: u64,
    pub mask: u64,
}

pub const SIG_DFL: u64 = 0;
pub const SIG_IGN: u64 = 1;

impl Default for SigAction {
    fn default() -> Self {
        Self {
            handler: SIG_DFL,
            flags: 0,
            mask: 0,
        }
    }
}
