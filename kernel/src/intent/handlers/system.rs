//! System Intent Handlers
//! 
//! Standard handlers for core system functionality (FileSystem, Help, Navigation).
//! These are registered as default handlers by the IntentExecutor.

use crate::intent::{Intent, IntentData};
use crate::intent::handlers::HandlerResult;
use crate::steno::dictionary::concepts;
use alloc::vec::Vec;
use crate::kprintln;

/// Handle file listing (LIST_FILES)
pub fn handle_list_files(_intent: &Intent) -> HandlerResult {
    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║            FILE LISTING               ║");
    kprintln!("╠═══════════════════════════════════════╣");
    
    let vfs = crate::fs::vfs::VFS.lock();
    match vfs.read_dir("/") {
            Ok(entries) => {
                for entry in entries {
                    let size = entry.size;
                    let type_str = if entry.is_dir { "<DIR>" } else { "     " };
                    // Truncate name to fit if needed
                    let name = if entry.name.len() > 15 { &entry.name[0..15] } else { &entry.name };
                    kprintln!("║ {} {:<15} {:>8} B ║", type_str, name, size);
                }
            }
            Err(e) => {
                kprintln!("║ [ERROR] Failed to list: {:<12} ║", e);
            }
    }

    kprintln!("╚═══════════════════════════════════════╝");
    kprintln!("[INTENT] LIST_FILES executed");
    HandlerResult::Handled
}

/// Handle reading a file (READ_FILE)
pub fn handle_read_file(intent: &Intent) -> HandlerResult {
    let filename = match &intent.data {
            IntentData::String(s) => s.as_str(),
            _ => {
                kprintln!("[INTENT] READ_FILE currently requires a filename (IntentData::String)");
                return HandlerResult::Error(1);
            }
    };

    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║ FILE: {:<51} ║", filename);
    kprintln!("╠═══════════════════════════════════════════════════════════╣");

    let vfs = crate::fs::vfs::VFS.lock();
    match vfs.open(filename, 0) { // 0 = O_RDONLY
        Ok(file_lock) => {
            let mut file = file_lock.lock();
            let size = file.stat().map(|s| s.size).unwrap_or(0);
            
            if size == 0 {
                kprintln!("║ [EMPTY FILE]                                              ║");
            } else if size > 4096 {
                    kprintln!("║ [ERROR] File too large for simple cat (limit 4KB)         ║");
            } else {
                // Read content
                let mut buf = alloc::vec![0u8; size as usize];
                match file.read(&mut buf) {
                    Ok(n) => {
                            if let Ok(s) = core::str::from_utf8(&buf[0..n]) {
                                for line in s.lines() {
                                    // Simple line printing, truncating if too long for box
                                    let chars = line.len();
                                    if chars > 58 {
                                        kprintln!("║ {:<58} ║", &line[0..58]);
                                        kprintln!("║ {:<58} ║", &line[58..]);
                                    } else {
                                        kprintln!("║ {:<58} ║", line);
                                    }
                                }
                            } else {
                                kprintln!("║ [BINARY DATA]                                             ║");
                            }
                    }
                    Err(_) => {
                        kprintln!("║ [ERROR] Read failed                                       ║");
                    }
                }
            }
        }
        Err(_) => {
            kprintln!("║ [ERROR] File not found                                    ║");
        }
    }
    kprintln!("╚═══════════════════════════════════════════════════════════╝");
    HandlerResult::Handled
}

pub fn handle_help(_intent: &Intent) -> HandlerResult {
    kprintln!("╔═══════════════════════════════════════╗");
    kprintln!("║     INTENT KERNEL - STENO HELP        ║");
    kprintln!("╠═══════════════════════════════════════╣");
    kprintln!("║ Strokes are semantic. Not characters. ║");
    kprintln!("║                                       ║");
    kprintln!("║ System:  STAT, PH-FPL (help), *       ║");
    kprintln!("║ Display: SHRO (show), HEU (hide)      ║");
    kprintln!("║ Memory:  STOR, RAOE/KAUL (recall)     ║");
    kprintln!("║ Files:   LIST_FILES, READ_FILE        ║");
    kprintln!("╚═══════════════════════════════════════╝");
    HandlerResult::Handled
}

pub fn handle_status(_intent: &Intent) -> HandlerResult {
    kprintln!("[SYSTEM] Status: OPERATIONAL");
    HandlerResult::Handled
}

pub fn handle_reboot(_intent: &Intent) -> HandlerResult {
    kprintln!("[SYSTEM] Rebooting...");
    // crate::arch::reboot(); // If available
    HandlerResult::Handled
}

pub fn handle_clear(_intent: &Intent) -> HandlerResult {
    // Clear screen ANSI code
    kprintln!("\x1b[2J\x1b[H");
    HandlerResult::Handled
}

pub fn handle_echo(intent: &Intent) -> HandlerResult {
    match &intent.data {
        IntentData::String(s) => kprintln!("{}", s),
        IntentData::Number(n) => kprintln!("{}", n),
        _ => kprintln!("(Empty Echo)"),
    }
    HandlerResult::Handled
}

pub fn handle_undo(_intent: &Intent) -> HandlerResult {
    kprintln!("[UNDO] Last action undone");
    HandlerResult::Handled
}

pub fn handle_show(_intent: &Intent) -> HandlerResult {
    kprintln!("[DISPLAY] Show");
    HandlerResult::Handled
}

pub fn handle_hide(_intent: &Intent) -> HandlerResult {
    kprintln!("[DISPLAY] Hide");
    HandlerResult::Handled
}

pub fn handle_store(_intent: &Intent) -> HandlerResult {
    // Capability check is handled by registry
    kprintln!("[MEMORY] Store");
    HandlerResult::Handled
}

pub fn handle_recall(_intent: &Intent) -> HandlerResult {
    kprintln!("[MEMORY] Recall");
    HandlerResult::Handled
}

pub fn handle_delete(_intent: &Intent) -> HandlerResult {
    // Capability check is handled by registry
    kprintln!("[MEMORY] Delete");
    HandlerResult::Handled
}

pub fn handle_next(_intent: &Intent) -> HandlerResult {
    kprintln!("[NAV] Next");
    HandlerResult::Handled
}

pub fn handle_previous(_intent: &Intent) -> HandlerResult {
    kprintln!("[NAV] Previous");
    HandlerResult::Handled
}

pub fn handle_back(_intent: &Intent) -> HandlerResult {
    kprintln!("[NAV] Back");
    HandlerResult::Handled
}

pub fn handle_yes(_intent: &Intent) -> HandlerResult {
    kprintln!("[CONFIRM] Yes");
    HandlerResult::Handled
}

pub fn handle_no(_intent: &Intent) -> HandlerResult {
    kprintln!("[CONFIRM] No");
    HandlerResult::Handled
}

pub fn handle_confirm(_intent: &Intent) -> HandlerResult {
    kprintln!("[CONFIRM] Confirmed");
    HandlerResult::Handled
}

pub fn handle_cancel(_intent: &Intent) -> HandlerResult {
    kprintln!("[CONFIRM] Cancelled");
    HandlerResult::Handled
}
