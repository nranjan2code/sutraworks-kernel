//! System Recovery Manager
//!
//! Centralizes error handling and recovery strategies.

use crate::kprintln;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Usb,
    SdCard,
    Network,
    Hailo,
    Display,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Transient, // Retryable
    Recoverable, // Needs reset
    Fatal, // Degrade or Panic
}

pub struct RecoveryManager;

impl RecoveryManager {
    pub fn report_error(component: Component, severity: ErrorSeverity, message: &str) {
        kprintln!("[RECOVERY] Error in {:?}: {} ({:?})", component, message, severity);
        
        match severity {
            ErrorSeverity::Transient => {
                // Log and continue
            },
            ErrorSeverity::Recoverable => {
                // Trigger reset if possible
                Self::recover_component(component);
            },
            ErrorSeverity::Fatal => {
                // Degrade or Panic
                Self::degrade_component(component);
            }
        }
    }
    
    fn recover_component(component: Component) {
        kprintln!("[RECOVERY] Attempting to recover {:?}...", component);
        match component {
            Component::Usb => {
                // Trigger USB reset
                // crate::drivers::usb::reset_controller(); // If exposed
            },
            Component::SdCard => {
                // crate::drivers::sd::reset();
            },
            Component::Network => {
                // crate::drivers::ethernet::reinit();
            },
            Component::Hailo => {
                // crate::drivers::hailo::reload_firmware();
            },
            _ => {}
        }
    }
    
    fn degrade_component(component: Component) {
        kprintln!("[RECOVERY] Degrading {:?}...", component);
        match component {
            Component::Hailo => {
                kprintln!("[RECOVERY] Switching to CPU Inference");
                // This is handled by PerceptionManager automatically if Hailo fails
            },
            Component::Display => {
                kprintln!("[RECOVERY] Switching to Serial Console");
                // Handled by Console driver
            },
            _ => {
                kprintln!("[RECOVERY] No degradation strategy for {:?}. System might be unstable.", component);
            }
        }
    }
}
