//! Response Generator - Intent results to natural language
//!
//! This module converts intent execution results into human-readable English responses.

use alloc::format;
use alloc::string::String;

use crate::intent::Intent;
use crate::steno::dictionary::concepts;
use crate::steno::EngineStats;

// ═══════════════════════════════════════════════════════════════════════════════
// RESULT TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of intent execution
#[derive(Clone)]
pub struct IntentResult {
    pub success: bool,
    pub data: ResultData,
    pub error: Option<&'static str>,
}

impl IntentResult {
    pub const fn success() -> Self {
        Self {
            success: true,
            data: ResultData::None,
            error: None,
        }
    }

    pub const fn with_data(data: ResultData) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    pub const fn error(msg: &'static str) -> Self {
        Self {
            success: false,
            data: ResultData::None,
            error: Some(msg),
        }
    }
}

/// Data payload in result
#[derive(Clone)]
pub enum ResultData {
    None,
    Stats(SystemStats),
    Message(&'static str),
    Number(i64),
}

/// System statistics for STATUS command
#[derive(Clone, Copy)]
pub struct SystemStats {
    pub cpu_usage: u8,          // 0-100%
    pub memory_used: u64,        // bytes
    pub memory_total: u64,       // bytes
    pub uptime_seconds: u64,
    pub steno: EngineStats,
}

// ═══════════════════════════════════════════════════════════════════════════════
// RESPONSE GENERATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Generates natural language responses from intent results
pub struct ResponseGenerator {
    pub verbose: bool,  // Detailed vs. concise output
}

impl ResponseGenerator {
    pub const fn new() -> Self {
        Self { verbose: false }
    }

    /// Get global generator instance
    pub fn global() -> &'static Self {
        static GENERATOR: ResponseGenerator = ResponseGenerator::new();
        &GENERATOR
    }

    /// Generate a response from intent result
    pub fn generate(&self, intent: &Intent, result: &IntentResult) -> String {
        // Handle errors first
        if !result.success {
            return self.error_response(intent, result.error);
        }

        // Route by concept
        match intent.concept_id {
            id if id == concepts::HELP => self.help_response(),
            id if id == concepts::STATUS => self.status_response(result),
            id if id == concepts::REBOOT => self.reboot_response(),
            id if id == concepts::CLEAR => String::from(""), // Silent
            id if id == concepts::UNDO => self.undo_response(),
            id if id == concepts::SHOW => self.show_response(),
            id if id == concepts::HIDE => self.hide_response(),
            id if id == concepts::SAVE => self.save_response(),
            id if id == concepts::LOAD => self.load_response(),
            id if id == concepts::SEARCH => self.search_response(),
            id if id == concepts::YES => self.yes_response(),
            id if id == concepts::NO => self.no_response(),
            id if id == concepts::CONFIRM => self.confirm_response(),
            id if id == concepts::CANCEL => self.cancel_response(),
            id if id == concepts::NEXT => self.next_response(),
            id if id == concepts::PREVIOUS => self.previous_response(),
            _ => self.unknown_response(intent),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // RESPONSE TEMPLATES
    // ═══════════════════════════════════════════════════════════════════════════

    fn help_response(&self) -> String {
        String::from(
            "Welcome to Intent Kernel!\n\
             \n\
             You can type naturally to control the system:\n\
             \n\
             System Commands:\n\
             • 'status' or 'how are you?' - Show system information\n\
             • 'help' or 'what can you do?' - Display this message\n\
             • 'clear' - Clear the screen\n\
             • 'reboot' - Restart the system\n\
             \n\
             Actions:\n\
             • 'show' - Display something\n\
             • 'hide' - Hide something\n\
             • 'save' - Save current state\n\
             • 'search' - Find something\n\
             \n\
             Confirmation:\n\
             • 'yes' or 'ok' - Confirm action\n\
             • 'no' or 'cancel' - Decline action\n\
             \n\
             Navigation:\n\
             • 'next' - Go forward\n\
             • 'previous' or 'back' - Go backward\n\
             \n\
             You can also use stenographic notation if you know it!\n\
             Type 'status' to see system health."
        )
    }

    fn status_response(&self, result: &IntentResult) -> String {
        match &result.data {
            ResultData::Stats(stats) => {
                if self.verbose {
                    self.detailed_status(stats)
                } else {
                    self.concise_status(stats)
                }
            }
            _ => String::from("System Status: OK"),
        }
    }

    fn concise_status(&self, stats: &SystemStats) -> String {
        let mem_pct = (stats.memory_used as f32 / stats.memory_total as f32 * 100.0) as u8;
        let uptime = format_duration(stats.uptime_seconds);

        format!(
            "System: CPU {}% | RAM {}% ({:.1}GB/{:.1}GB) | Up {}\n\
             Steno: {} strokes | {} intents | {} WPM",
            stats.cpu_usage,
            mem_pct,
            stats.memory_used as f64 / 1024.0 / 1024.0 / 1024.0,
            stats.memory_total as f64 / 1024.0 / 1024.0 / 1024.0,
            uptime,
            stats.steno.strokes_processed,
            stats.steno.intents_matched,
            calculate_wpm(&stats.steno, stats.uptime_seconds)
        )
    }

    fn detailed_status(&self, stats: &SystemStats) -> String {
        let mem_pct = (stats.memory_used as f32 / stats.memory_total as f32 * 100.0) as u8;
        let uptime = format_duration(stats.uptime_seconds);

        format!(
            "System Status\n\
             ═══════════════════════════════════════\n\
             \n\
             Performance:\n\
             • CPU Usage: {}%\n\
             • Memory: {:.2}GB / {:.2}GB ({}% used)\n\
             • Uptime: {}\n\
             \n\
             Stenographic Engine:\n\
             • Strokes Processed: {}\n\
             • Intents Recognized: {}\n\
             • Corrections Made: {}\n\
             • Unrecognized: {}\n\
             • Average WPM: {}\n\
             • Accuracy: {:.1}%\n\
             \n\
             Status: All systems operational ✓",
            stats.cpu_usage,
            stats.memory_used as f64 / 1024.0 / 1024.0 / 1024.0,
            stats.memory_total as f64 / 1024.0 / 1024.0 / 1024.0,
            mem_pct,
            uptime,
            stats.steno.strokes_processed,
            stats.steno.intents_matched,
            stats.steno.corrections,
            stats.steno.unrecognized,
            calculate_wpm(&stats.steno, stats.uptime_seconds),
            calculate_accuracy(&stats.steno)
        )
    }

    fn reboot_response(&self) -> String {
        String::from("Rebooting system... Please wait.")
    }

    fn undo_response(&self) -> String {
        String::from("Last action undone.")
    }

    fn show_response(&self) -> String {
        String::from("Displaying...")
    }

    fn hide_response(&self) -> String {
        String::from("Hidden.")
    }

    fn save_response(&self) -> String {
        String::from("Saved successfully.")
    }

    fn load_response(&self) -> String {
        String::from("Loaded successfully.")
    }

    fn search_response(&self) -> String {
        String::from("Searching...")
    }

    fn yes_response(&self) -> String {
        String::from("Confirmed.")
    }

    fn no_response(&self) -> String {
        String::from("Cancelled.")
    }

    fn confirm_response(&self) -> String {
        String::from("Action confirmed.")
    }

    fn cancel_response(&self) -> String {
        String::from("Action cancelled.")
    }

    fn next_response(&self) -> String {
        String::from("Moving forward...")
    }

    fn previous_response(&self) -> String {
        String::from("Going back...")
    }

    fn unknown_response(&self, intent: &Intent) -> String {
        format!("Executed: {}", intent.name)
    }

    fn error_response(&self, intent: &Intent, error: Option<&'static str>) -> String {
        if let Some(msg) = error {
            format!("Error: {}", msg)
        } else {
            format!("Failed to execute: {}", intent.name)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Format duration in human-readable form
fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Calculate WPM from steno stats
fn calculate_wpm(stats: &EngineStats, uptime_seconds: u64) -> u64 {
    if uptime_seconds == 0 {
        return 0;
    }

    // Assume average 5 strokes per word
    let words = stats.strokes_processed / 5;
    let minutes = uptime_seconds / 60;

    if minutes == 0 {
        return 0;
    }

    words / minutes
}

/// Calculate accuracy percentage
fn calculate_accuracy(stats: &EngineStats) -> f32 {
    if stats.strokes_processed == 0 {
        return 100.0;
    }

    let successful = stats.intents_matched;
    let total = stats.strokes_processed;

    (successful as f32 / total as f32) * 100.0
}

// ═══════════════════════════════════════════════════════════════════════════════
// TRAIT IMPLEMENTATIONS
// ═══════════════════════════════════════════════════════════════════════════════

impl Default for ResponseGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::IntentData;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn test_calculate_wpm() {
        let stats = EngineStats {
            strokes_processed: 100,
            intents_matched: 95,
            corrections: 5,
            unrecognized: 0,
            multi_stroke_matches: 0,
        };

        // 100 strokes / 5 = 20 words
        // 60 seconds = 1 minute
        // 20 words / 1 minute = 20 WPM
        assert_eq!(calculate_wpm(&stats, 60), 20);
    }

    #[test]
    fn test_calculate_accuracy() {
        let stats = EngineStats {
            strokes_processed: 100,
            intents_matched: 95,
            corrections: 5,
            unrecognized: 0,
            multi_stroke_matches: 0,
        };

        assert_eq!(calculate_accuracy(&stats), 95.0);
    }

    #[test]
    fn test_help_response() {
        let gen = ResponseGenerator::new();
        let response = gen.help_response();
        assert!(response.contains("Welcome"));
        assert!(response.contains("status"));
    }

    #[test]
    fn test_error_response() {
        let gen = ResponseGenerator::new();
        let intent = Intent {
            concept_id: concepts::HELP,
            confidence: 1.0,
            data: IntentData::None,
            name: "HELP",
        };

        let response = gen.error_response(&intent, Some("Test error"));
        assert!(response.contains("Error"));
        assert!(response.contains("Test error"));
    }
}
