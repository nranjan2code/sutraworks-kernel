use alloc::sync::Arc;
use alloc::string::String;
use alloc::format;
use crate::apps::registry::{Skill, Context, SkillError};
use crate::intent::ConceptID;

// --- Skill: Identify Person ---

pub struct IdentifyPersonSkill;

impl Skill for IdentifyPersonSkill {
    fn name(&self) -> &str {
        "Identify Person"
    }
    
    fn description(&self) -> &str {
        "Identifies a person from an image input"
    }
    
    fn semantic_tag(&self) -> ConceptID {
        ConceptID::new(0x1001) // Identiy Person ID
    }

    fn execute(&self, input: &str, _ctx: &Context) -> Result<String, SkillError> {
        // Mock implementation
        if input.contains("Face detected") {
            Ok("Authorized User".into())
        } else {
            Err(SkillError::InvalidInput)
        }
    }
}

// --- Skill: Unlock Door ---

pub struct UnlockDoorSkill;

impl Skill for UnlockDoorSkill {
    fn name(&self) -> &str {
        "Unlock Door"
    }
    
    fn description(&self) -> &str {
        "Unlocks the physical door mechanism"
    }
    
    fn semantic_tag(&self) -> ConceptID {
        ConceptID::new(0x1002) // Unlock Door ID
    }

    fn execute(&self, input: &str, _ctx: &Context) -> Result<String, SkillError> {
        if input == "Authorized User" {
            crate::kprintln!("[DEVICE] Door Solenoid Activated: CROSSBAR UNLOCKED");
            Ok("Success".into())
        } else {
            crate::kprintln!("[DEVICE] Access Denied for: {}", input);
            Err(SkillError::ExecutionFailed)
        }
    }
}

pub fn register_demo_skills() {
    let mut registry = crate::apps::registry::REGISTRY.lock();
    registry.register(Arc::new(IdentifyPersonSkill));
    registry.register(Arc::new(UnlockDoorSkill));
}
