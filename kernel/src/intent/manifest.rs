//! Intent Manifest - The DNA of an Intent-Native Application
//!
//! Defines the structure of an application as a graph of intents.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::intent::ConceptID;

/// A declarative manifest defining an application
#[derive(Clone, Debug)]
pub struct IntentManifest {
    pub app_name: String,
    pub triggers: Vec<Trigger>,
    pub flow: Vec<FlowStep>,
}

/// A trigger that maps user input to an intent
#[derive(Clone, Debug)]
pub struct Trigger {
    pub input_pattern: String,
    pub intent: ConceptID,
}

/// A step in the execution flow
#[derive(Clone, Debug)]
pub struct FlowStep {
    pub on_intent: ConceptID,
    pub action: String,
    pub parameters: Vec<String>,
}

impl IntentManifest {
    pub fn new(name: &str) -> Self {
        Self {
            app_name: name.to_string(),
            triggers: Vec::new(),
            flow: Vec::new(),
        }
    }

    pub fn add_trigger(mut self, pattern: &str, intent: ConceptID) -> Self {
        self.triggers.push(Trigger {
            input_pattern: pattern.to_string(),
            intent,
        });
        self
    }

    pub fn add_step(mut self, on_intent: ConceptID, action: &str, params: &[&str]) -> Self {
        let parameters = params.iter().map(|s| s.to_string()).collect();
        self.flow.push(FlowStep {
            on_intent,
            action: action.to_string(),
            parameters,
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_builder() {
        let manifest = IntentManifest::new("Calorie Tracker")
            .add_trigger("I ate a [Food]", ConceptID::new(1))
            .add_step(ConceptID::new(1), "Log Food", &["Food"]);

        assert_eq!(manifest.app_name, "Calorie Tracker");
        assert_eq!(manifest.triggers.len(), 1);
        assert_eq!(manifest.flow.len(), 1);
        assert_eq!(manifest.triggers[0].input_pattern, "I ate a [Food]");
    }
}
