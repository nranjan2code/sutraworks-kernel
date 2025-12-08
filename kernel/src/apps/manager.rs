//! App Manager - Manages the lifecycle of Intent Apps
//!
//! Handles loading manifests, registering triggers, and managing active applications.

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::collections::BTreeMap;
use alloc::format;
use spin::Mutex;
use crate::apps::manifest::{AppManifest, FlowStep};
use crate::apps::registry::{Context, SkillError};
use crate::apps::linker::SemanticLinker;
use crate::intent::ConceptID;
use crate::kprintln;

/// Manages all intent applications in the system
pub struct AppManager {
    apps: Vec<AppManifest>,
}

impl AppManager {
    pub const fn new() -> Self {
        Self {
            apps: Vec::new(),
        }
    }

    /// Loads an app from a manifest string
    pub fn load_from_string(&mut self, source: &str) -> Result<(), &'static str> {
        match AppManifest::parse(source) {
            Ok(manifest) => {
                kprintln!("[APP] Loaded: {}", manifest.app_name);
                self.apps.push(manifest);
                Ok(())
            },
            Err(_) => Err("Failed to parse manifest"),
        }
    }

    /// Finds an app that has a trigger matching the input pattern
    /// Returns the app and the specific flow to execute
    pub fn find_trigger(&self, input: &str) -> Option<(&AppManifest, &crate::apps::manifest::Trigger)> {
        for app in &self.apps {
            for trigger in &app.triggers {
                // Simple case-insensitive match for now
                // In production, this would use the Neural Matcher / HDC
                if input.eq_ignore_ascii_case(&trigger.input_pattern) {
                    return Some((app, trigger));
                }
            }
        }
        None
    }

    pub fn list_apps(&self) {
        kprintln!("--- Installed Apps ---");
        for app in &self.apps {
            kprintln!("- {}: {}", app.app_name, app.description);
        }
        kprintln!("----------------------");
    }
    pub fn execute_app(&self, app: &AppManifest, trigger_input: &str) -> Result<(), &'static str> {
        kprintln!("[APP] Executing: {}", app.app_name);
        
        // Execution context (variables)
        let mut variables: BTreeMap<String, String> = BTreeMap::new();
        variables.insert("trigger.input".to_string(), trigger_input.to_string());
        
        let ctx = Context::default(); // User context

        for step in &app.flow {
            kprintln!("[APP] Step {}: {}", step.id, step.goal);
            
            // 1. Resolve Skill
            let skill = SemanticLinker::resolve(&step.goal)
                .ok_or("Failed to resolve skill for step")?;
                
            kprintln!("[APP]   -> Bound to Skill: {}", skill.name());
            
            // 2. Resolve Inputs
            // For now, take the first input variable or raw string
            let input_val = if let Some(first_input) = step.inputs.first() {
                if first_input.starts_with('$') {
                    // Variable lookup
                    let var_name = &first_input[1..];
                    variables.get(var_name).map(|s| s.as_str()).unwrap_or("")
                } else {
                    // Literal
                    first_input.as_str()
                }
            } else {
                ""
            };
            
            // 3. Execute Candidate
            match skill.execute(input_val, &ctx) {
                Ok(output) => {
                    kprintln!("[APP]   -> Result: {}", output);
                    // Store result for future steps
                    variables.insert(format!("{}.result", step.id), output);
                },
                Err(e) => {
                    kprintln!("[APP]   -> Error: {:?}", e);
                    return Err("Step execution failed");
                }
            }
        }
        
        kprintln!("[APP] Execution Complete");
        Ok(())
    }
}

// Global App Manager instance
pub static APP_MANAGER: Mutex<AppManager> = Mutex::new(AppManager::new());
