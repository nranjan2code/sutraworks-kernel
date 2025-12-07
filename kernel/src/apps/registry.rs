use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use spin::Mutex;
use crate::intent::ConceptID;
use crate::kernel::process::{AgentId, Message};
use crate::kernel::scheduler::SCHEDULER;

/// Context passed to a skill during execution.
#[derive(Debug, Clone, Default)]
pub struct Context {
    // Placeholder for now. Could contain user ID, session info, etc.
    pub user_id: u64,
}

#[derive(Debug)]
pub enum SkillError {
    ExecutionFailed,
    InvalidInput,
}

/// The atomic unit of capability in the Intent Kernel.
/// Skills are "verbs" that can be executed.
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    
    // In a real system, this would return a vector embedding.
    // For now, we'll return a ConceptID that "tags" this skill.
    fn semantic_tag(&self) -> ConceptID;

    fn execute(&self, input: &str, ctx: &Context) -> Result<String, SkillError>;
}

/// A skill backed by a running process (Semantic Binding).
pub struct ProcessSkill {
    pub pid: u64,
    pub concept: ConceptID,
}

impl Skill for ProcessSkill {
    fn name(&self) -> &str { "Process Capability" }
    fn description(&self) -> &str { "External Process Handler" }
    fn semantic_tag(&self) -> ConceptID { self.concept }
    
    fn execute(&self, input: &str, _ctx: &Context) -> Result<String, SkillError> {
        let mut scheduler = SCHEDULER.lock();
        
        // Construct Message
        let mut data = [0u8; 64];
        let bytes = input.as_bytes();
        let len = core::cmp::min(bytes.len(), 64);
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), data.as_mut_ptr(), len);
        }
        
        // Sender is current process (the one calling execute/syscall)
        let sender = scheduler.current_pid().map(|p| AgentId(p)).unwrap_or(AgentId(0));
        
        let msg = Message {
            sender,
            data,
        };
        
        if scheduler.send_message(self.pid, msg).is_ok() {
            Ok("Intent Dispatched".into())
        } else {
            Err(SkillError::ExecutionFailed)
        }
    }
}

/// A registry of all available skills in the system.
pub struct SkillRegistry {
    skills: Vec<Arc<dyn Skill>>,
}

impl SkillRegistry {
    pub const fn new() -> Self {
        Self {
            skills: Vec::new(),
        }
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        self.skills.push(skill);
    }
    
    pub fn register_pid(&mut self, concept: ConceptID, pid: u64) {
        let skill = Arc::new(ProcessSkill { pid, concept });
        self.register(skill);
    }
    
    pub fn find_by_tag(&self, tag: ConceptID) -> Option<Arc<dyn Skill>> {
        for skill in &self.skills {
            if skill.semantic_tag() == tag {
                return Some(skill.clone());
            }
        }
        None
    }
    
    pub fn all_skills(&self) -> &[Arc<dyn Skill>] {
        &self.skills
    }
}

// Global registry instance
pub static REGISTRY: Mutex<SkillRegistry> = Mutex::new(SkillRegistry::new());
