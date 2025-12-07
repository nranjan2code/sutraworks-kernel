use alloc::sync::Arc;
use alloc::string::String;
use crate::apps::registry::{Skill, REGISTRY};
use crate::intent::ConceptID;

pub struct SemanticLinker;

impl SemanticLinker {
    /// Resolves a declared intent (from a manifest) to an executable Skill.
    /// 
    /// In a full implementation, this would use vector similarity search.
    /// For this version, we'll use simple keyword matching or direct ConceptID mapping.
    pub fn resolve(goal: &str) -> Option<Arc<dyn Skill>> {
        let registry = REGISTRY.lock();
        
        // Simple heuristic for V1:
        // Check if any skill description contains the goal words
        // OR if the goal string matches the skill name.
        
        for skill in registry.all_skills() {
            if skill.name().eq_ignore_ascii_case(goal) {
                return Some(skill.clone());
            }
            if skill.description().contains(goal) {
                return Some(skill.clone());
            }
        }
        
        None
    }
}
