use std::sync::Arc;
use std::string::String;
use std::vec::Vec;

// Mock structures to mirror kernel implementation
#[derive(Debug, Clone, PartialEq)]
pub struct ConceptID(pub u64);

#[derive(Debug, Clone)]
pub struct Context {
    pub user_id: u64,
}

pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn semantic_tag(&self) -> ConceptID;
    fn execute(&self, input: &str, ctx: &Context) -> Result<String, String>;
}

pub struct SkillRegistry {
    skills: Vec<Arc<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: Vec::new(),
        }
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        self.skills.push(skill);
    }
    
    pub fn all_skills(&self) -> &[Arc<dyn Skill>] {
        &self.skills
    }
}

pub struct SemanticLinker;

impl SemanticLinker {
    pub fn resolve(registry: &SkillRegistry, goal: &str) -> Option<Arc<dyn Skill>> {
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

// Mock Skill
struct MockSkill;
impl Skill for MockSkill {
    fn name(&self) -> &str { "Mock Skill" }
    fn description(&self) -> &str { "Does mock things" }
    fn semantic_tag(&self) -> ConceptID { ConceptID(1) }
    fn execute(&self, _input: &str, _ctx: &Context) -> Result<String, String> {
        Ok("Executed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_resolve() {
        let mut registry = SkillRegistry::new();
        let skill = Arc::new(MockSkill);
        registry.register(skill);
        
        // Test resolution by name
        let resolved = SemanticLinker::resolve(&registry, "Mock Skill");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().name(), "Mock Skill");
        
        // Test resolution by description
        let resolved_desc = SemanticLinker::resolve(&registry, "mock things");
        assert!(resolved_desc.is_some());
        
        // Test execution
        let ctx = Context { user_id: 1 };
        let result = resolved_desc.unwrap().execute("test", &ctx);
        assert_eq!(result.unwrap(), "Executed");
    }
}
