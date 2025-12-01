//! Semantic Intent Engine
//!
//! A forward-looking, concept-based engine that understands intent through
//! semantic hashing and context, rather than rigid string parsing.

use core::str;
use crate::kernel::capability::{
    Capability, CapabilityType, Permissions, 
    mint_root, validate
};

pub mod embeddings;
use embeddings::*;
use alloc::string::String;

// ═══════════════════════════════════════════════════════════════════════════════
// SEMANTIC CONCEPTS
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// SEMANTIC CONCEPTS
// ═══════════════════════════════════════════════════════════════════════════════

/// A 64-bit semantic hash representing a concept ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConceptID(pub u64);

/// A Real Vector Embedding (64 dimensions, quantized to i8)
/// 
/// This allows for efficient storage and SIMD-friendly calculations.
/// Range: -128 to 127.
#[derive(Clone, Copy, Debug)]
pub struct Embedding {
    pub id: ConceptID,
    pub vector: [i8; 64], 
}

impl ConceptID {
    /// Create a ConceptID from a string slice (FNV-1a hash)
    pub const fn from_str(s: &str) -> Self {
        let mut hash: u64 = 0xcbf29ce484222325;
        let bytes = s.as_bytes();
        let mut i = 0;
        
        while i < bytes.len() {
            hash ^= bytes[i] as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            i += 1;
        }
        
        ConceptID(hash)
    }
}

impl Embedding {
    /// Create a new embedding with specific vector data
    pub const fn new(id: ConceptID, vector: [i8; 64]) -> Self {
        Embedding { id, vector }
    }

    /// Calculate Cosine Similarity
    /// 
    /// Formula: (A . B) / (||A|| * ||B||)
    /// Returns a score from 0 to 100 (mapped from -1.0 to 1.0)
    pub fn similarity(&self, other: &Embedding) -> u8 {
        let mut dot: i32 = 0;
        let mut mag_a_sq: i32 = 0;
        let mut mag_b_sq: i32 = 0;
        
        for i in 0..64 {
            let a = self.vector[i] as i32;
            let b = other.vector[i] as i32;
            
            dot += a * b;
            mag_a_sq += a * a;
            mag_b_sq += b * b;
        }
        
        if mag_a_sq == 0 || mag_b_sq == 0 { return 0; }
        
        // Integer Square Root approximation
        let mag_a = isqrt(mag_a_sq as u32) as i32;
        let mag_b = isqrt(mag_b_sq as u32) as i32;
        
        if mag_a == 0 || mag_b == 0 { return 0; }
        
        let denom = mag_a * mag_b;
        
        // Scale dot product to avoid precision loss before division
        // We want result * 100
        // (dot * 100) / denom
        
        let score = (dot * 100) / denom;
        
        // Clamp to 0-100 (handle negative correlation as 0 for this use case)
        if score < 0 { 0 } else { score.min(100) as u8 }
    }
}

/// Integer Square Root (Newton's method)
fn isqrt(n: u32) -> u32 {
    if n < 2 { return n; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Neural Memory: A vector database for concepts
pub struct NeuralMemory {
    concepts: [Option<Embedding>; 64], // Fixed size for no-std demo
    count: usize,
}

impl NeuralMemory {
    pub const fn new() -> Self {
        NeuralMemory {
            concepts: [None; 64],
            count: 0,
        }
    }
    
    pub fn remember(&mut self, embedding: Embedding) {
        if self.count < 64 {
            self.concepts[self.count] = Some(embedding);
            self.count += 1;
        }
    }
    
    pub fn recall(&self, query: &Embedding) -> Option<ConceptID> {
        let mut best_match = None;
        let mut best_score = 0;
        
        for i in 0..self.count {
            if let Some(emb) = &self.concepts[i] {
                let score = query.similarity(emb);
                if score > best_score {
                    best_score = score;
                    best_match = Some(emb.id);
                }
            }
        }
        
        // Threshold for recognition
        if best_score > 70 { best_match } else { None }
    }
}

// Core Concepts
pub const CONCEPT_DISPLAY: ConceptID = ConceptID::from_str("display");
pub const CONCEPT_STORE: ConceptID = ConceptID::from_str("store");
pub const CONCEPT_RETRIEVE: ConceptID = ConceptID::from_str("retrieve");
pub const CONCEPT_COMPUTE: ConceptID = ConceptID::from_str("compute");
pub const CONCEPT_SYSTEM: ConceptID = ConceptID::from_str("system");
pub const CONCEPT_QUERY: ConceptID = ConceptID::from_str("query");

// Action Concepts
pub const ACTION_SHOW: ConceptID = ConceptID::from_str("show");
pub const ACTION_CALCULATE: ConceptID = ConceptID::from_str("calculate");
pub const ACTION_REBOOT: ConceptID = ConceptID::from_str("reboot");
pub const ACTION_STORE: ConceptID = ConceptID::from_str("store");
pub const ACTION_RETRIEVE: ConceptID = ConceptID::from_str("retrieve");
pub const ACTION_LIST: ConceptID = ConceptID::from_str("list");
pub const ACTION_READ: ConceptID = ConceptID::from_str("read");
pub const ACTION_LOAD: ConceptID = ConceptID::from_str("load");
pub const ACTION_CREATE: ConceptID = ConceptID::from_str("create");
pub const ACTION_EDIT: ConceptID = ConceptID::from_str("edit");
pub const ACTION_DELETE: ConceptID = ConceptID::from_str("delete");

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// A semantic intent
#[derive(Clone, Debug)]
pub struct Intent {
    pub primary_concept: ConceptID,
    pub action_concept: ConceptID,
    pub target: Option<ConceptID>,
    pub data: IntentData,
    pub confidence: u8, // 0-100
}

#[derive(Clone, Debug)]
pub enum IntentData {
    None,
    Text(String), // Dynamic string
    Number(i64),
    Raw(u64),
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONTEXT AWARENESS
// ═══════════════════════════════════════════════════════════════════════════════

const CONTEXT_DEPTH: usize = 5;

#[derive(Clone)]
pub struct Context {
    pub history: [Option<Intent>; CONTEXT_DEPTH],
    pub last_result: Option<IntentData>,
    pub active_topic: Option<ConceptID>,
}

impl Context {
    pub const fn new() -> Self {
        Context {
            history: [const { None }; CONTEXT_DEPTH],
            last_result: None,
            active_topic: None,
        }
    }

    pub fn push(&mut self, intent: Intent) {
        // Shift history
        for i in (1..CONTEXT_DEPTH).rev() {
            self.history[i] = self.history[i-1].take();
        }
        self.history[0] = Some(intent);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SEMANTIC ENGINE
// ═══════════════════════════════════════════════════════════════════════════════

pub struct SemanticEngine {
    context: Context,
    // Capabilities
    display_cap: Option<Capability>,
    memory_cap: Option<Capability>,
    system_cap: Option<Capability>,
    compute_cap: Option<Capability>,
}

impl SemanticEngine {
    pub const fn new() -> Self {
        SemanticEngine {
            context: Context::new(),
            display_cap: None,
            memory_cap: None,
            system_cap: None,
            compute_cap: None,
        }
    }

    pub fn has_capability(&self, cap_type: crate::kernel::capability::CapabilityType) -> bool {
        match cap_type {
            crate::kernel::capability::CapabilityType::System => {
                self.system_cap.as_ref().map(|c| c.is_valid()).unwrap_or(false)
            },
            // Add others as needed
            _ => false,
        }
    }

    pub fn init(&mut self) {
        // Mint root capabilities
        unsafe {
            self.display_cap = mint_root(CapabilityType::Display, 0, 0, Permissions::ALL);
            self.memory_cap = mint_root(CapabilityType::Memory, 0, 0x1_0000_0000, Permissions::ALL);
            self.system_cap = mint_root(CapabilityType::System, 0, 0, Permissions::ALL);
            self.compute_cap = mint_root(CapabilityType::Compute, 0, 0, Permissions::ALL);
        }
    }

    /// Process natural language into a semantic intent
    /// 
    /// Uses a static vector database to map input tokens to concepts.
    pub fn understand(&mut self, input: &str) -> Option<Intent> {
        let input = input.trim();
        if input.is_empty() { return None; }

        // Tokenize (simplistic)
        let mut tokens = input.split_whitespace();
        let first = tokens.next()?;
        
        // In a real system, we would run an embedding model here to get the vector for `first`.
        // Since we don't have an NPU driver yet, we simulate the "Embedding Model" step
        // by looking up the token in a static dictionary of known embeddings.
        // If unknown, we generate a random-ish vector (which won't match anything).
        
        let query_vec = get_static_embedding(first);
        
        // Now perform Vector Search (Cosine Similarity) against known concepts
        // This is the "Neural Memory" recall step.
        let concept = self.recall_concept(&query_vec);
        
        let (primary, action) = match concept {
            Some(CONCEPT_DISPLAY) => (CONCEPT_DISPLAY, ACTION_SHOW),
            Some(CONCEPT_COMPUTE) => (CONCEPT_COMPUTE, ACTION_CALCULATE),
            Some(CONCEPT_SYSTEM) => (CONCEPT_SYSTEM, ACTION_REBOOT),
            Some(CONCEPT_STORE) => (CONCEPT_STORE, ACTION_STORE),
            Some(CONCEPT_RETRIEVE) => (CONCEPT_RETRIEVE, ACTION_RETRIEVE),
            // Map "ls", "list", "dir" to LIST
            _ if first == "ls" || first == "list" || first == "dir" => (CONCEPT_SYSTEM, ACTION_LIST),
            // Map "cat", "read", "show" (file) to READ
            _ if first == "cat" || first == "read" => (CONCEPT_SYSTEM, ACTION_READ),
            // Map "load" to LOAD
            _ if first == "load" => (CONCEPT_SYSTEM, ACTION_LOAD),
            // Map "create", "touch", "make" to CREATE
            _ if first == "create" || first == "touch" || first == "make" => (CONCEPT_SYSTEM, ACTION_CREATE),
            // Map "write", "edit" to EDIT
            _ if first == "write" || first == "edit" => (CONCEPT_SYSTEM, ACTION_EDIT),
            // Map "delete", "rm", "remove" to DELETE
            _ if first == "delete" || first == "rm" || first == "remove" => (CONCEPT_SYSTEM, ACTION_DELETE),
            _ => (ConceptID::from_str("unknown"), ConceptID::from_str("unknown")),
        };

        // Extract data
        // We want to capture the rest of the line as arguments
        // Find where the first token ends
        let remainder = if let Some(idx) = input.find(char::is_whitespace) {
            input[idx..].trim()
        } else {
            ""
        };

        let data = if !remainder.is_empty() {
            if let Ok(n) = remainder.parse::<i64>() {
                IntentData::Number(n)
            } else {
                IntentData::Text(String::from(remainder)) 
            }
        } else {
            IntentData::None
        };

        Some(Intent {
            primary_concept: primary,
            action_concept: action,
            target: None,
            data,
            confidence: 90,
        })
    }
    
    fn recall_concept(&self, query: &Embedding) -> Option<ConceptID> {
        // Define our "Knowledge Base" of concepts with their vectors
        // In reality, this would be in the NeuralMemory struct, but for this demo
        // we define them here to keep the static data close to usage.
        
        let concepts = [
            (CONCEPT_DISPLAY, VEC_DISPLAY),
            (CONCEPT_COMPUTE, VEC_COMPUTE),
            (CONCEPT_SYSTEM, VEC_SYSTEM),
            (CONCEPT_STORE, VEC_STORE),
            (CONCEPT_RETRIEVE, VEC_RETRIEVE),
        ];
        
        let mut best_match = None;
        let mut best_score = 0;
        
        for (id, vec_data) in concepts.iter() {
            let target_emb = Embedding::new(*id, *vec_data);
            let score = query.similarity(&target_emb);
            
            // crate::kprintln!("Debug: Similarity with {:?} = {}", id, score);
            
            if score > best_score {
                best_score = score;
                best_match = Some(*id);
            }
        }
        
        if best_score > 60 { best_match } else { None }
    }

    pub fn execute(&mut self, intent: Intent) {
        // Update context
        self.context.push(intent.clone());

        match intent.primary_concept {
            CONCEPT_DISPLAY => self.handle_display(intent),
            CONCEPT_COMPUTE => self.handle_compute(intent),
            CONCEPT_SYSTEM => self.handle_system(intent),
            CONCEPT_STORE => self.handle_store(intent),
            CONCEPT_RETRIEVE => self.handle_retrieve(intent),
            _ => crate::kprintln!("? I sense the concept {:?}, but I don't know how to act on it.", intent.primary_concept),
        }
    }

    fn handle_display(&self, intent: Intent) {
        if let Some(cap) = &self.display_cap {
            if validate(cap) {
                match intent.data {
                    IntentData::Number(n) => crate::kprintln!("DISPLAY: {}", n),
                    IntentData::Text(s) => crate::kprintln!("DISPLAY: {}", s),
                    _ => crate::kprintln!("DISPLAY: [Empty]"),
                }
                return;
            }
        }
        crate::kprintln!("✗ Permission denied for Display");
    }

    fn handle_compute(&mut self, intent: Intent) {
        if let Some(cap) = &self.compute_cap {
            if validate(cap) {
                if let IntentData::Number(n) = intent.data {
                    // Check context for "double it" type logic
                    // For this demo, just square it
                    let result = n * n;
                    crate::kprintln!("COMPUTE: {}² = {}", n, result);
                    self.context.last_result = Some(IntentData::Number(result));
                }
                return;
            }
        }
        crate::kprintln!("✗ Permission denied for Compute");
    }

    fn handle_system(&self, intent: Intent) {
        if let Some(cap) = &self.system_cap {
            if validate(cap) {
                if intent.action_concept == ACTION_REBOOT {
                    crate::kprintln!("SYSTEM: Rebooting...");
                } else if intent.action_concept == ACTION_LIST {
                    self.handle_ls();
                } else if intent.action_concept == ACTION_READ {
                    self.handle_cat(intent);
                } else if intent.action_concept == ACTION_LOAD {
                    self.handle_load(intent);
                } else if intent.action_concept == ACTION_CREATE {
                    self.handle_create(intent);
                } else if intent.action_concept == ACTION_EDIT {
                    self.handle_edit(intent);
                } else if intent.action_concept == ACTION_DELETE {
                    self.handle_rm(intent);
                }
                return;
            }
        }
        crate::kprintln!("✗ Permission denied for System");
    }

    fn handle_store(&mut self, intent: Intent) {
        if let IntentData::Text(s) = intent.data {
            // Use content as embedding for now
            let embedding = get_static_embedding(&s); 
            
            // Allocate via Neural Allocator
            if let Some(ptr) = unsafe { 
                crate::kernel::memory::neural::NEURAL_ALLOCATOR.lock().alloc(s.len(), embedding) 
            } {
                // Copy data
                unsafe {
                    core::ptr::copy_nonoverlapping(s.as_ptr(), ptr.ptr.as_ptr(), s.len());
                }
                crate::kprintln!("STORE: Remembered '{}' as Concept {:?}", s, ptr.id);
            } else {
                crate::kprintln!("STORE: Failed (Out of Memory)");
            }
        } else {
             crate::kprintln!("STORE: What should I remember?");
        }
    }

    fn handle_retrieve(&mut self, intent: Intent) {
        if let IntentData::Text(s) = intent.data {
            let query = get_static_embedding(&s);
            if let Some(ptr) = unsafe {
                crate::kernel::memory::neural::NEURAL_ALLOCATOR.lock().retrieve(&query)
            } {
                crate::kprintln!("RETRIEVE: Found Concept {:?} (Semantic Match)", ptr.id);
            } else {
                crate::kprintln!("RETRIEVE: Nothing found matching '{}'", s);
            }
        } else {
            crate::kprintln!("RETRIEVE: What are you looking for?");
        }
    }

    fn handle_ls(&self) {
        use crate::fs::FileSystem;
        if let Some(fs) = crate::fs::get().as_ref() {
            if let Ok(files) = fs.list_files() {
                crate::kprintln!("FILESYSTEM:");
                for file in files {
                    let type_char = if file.is_dir { 'd' } else { '-' };
                    crate::kprintln!("{} {} {} bytes", type_char, file.name.as_str(), file.size);
                }
            } else {
                crate::kprintln!("Error listing files.");
            }
        } else {
            crate::kprintln!("No filesystem mounted.");
        }
    }

    fn handle_cat(&self, intent: Intent) {
        use crate::fs::FileSystem;
        if let IntentData::Text(filename) = intent.data {
             if let Some(fs) = crate::fs::get().as_ref() {
                let mut buffer = [0u8; 1024]; // Small buffer for demo
                if let Ok(size) = fs.read_file(filename.as_str(), &mut buffer) {
                    if let Ok(s) = core::str::from_utf8(&buffer[..size]) {
                        crate::kprintln!("CONTENTS of {}:", filename);
                        crate::kprintln!("{}", s);
                    } else {
                        crate::kprintln!("(Binary file)");
                    }
                } else {
                    crate::kprintln!("File not found: {}", filename);
                }
             } else {
                 crate::kprintln!("No filesystem mounted.");
             }
        } else {
            crate::kprintln!("Usage: cat <filename>");
        }
    }

    fn handle_load(&self, intent: Intent) {
        if let IntentData::Text(filename) = intent.data {
            crate::kprintln!("Loading {} into Neural Memory...", filename);
            // In future: read file, parse, store in NeuralMemory
            crate::kprintln!("(Simulated) Loaded 128 embeddings from {}", filename);
        }
    }

    fn handle_create(&self, intent: Intent) {
        // Security Check: Requires System Capability
        if !self.has_capability(crate::kernel::capability::CapabilityType::System) {
            crate::kprintln!("Access Denied: Requires System Capability");
            return;
        }

        use crate::fs::FileSystem;
        if let IntentData::Text(filename) = intent.data {
            if let Some(fs) = crate::fs::get().as_mut() {
                if let Err(e) = fs.create_file(filename.as_str()) {
                    crate::kprintln!("Error creating file: {}", e);
                } else {
                    crate::kprintln!("Created file: {}", filename);
                }
            } else {
                crate::kprintln!("No filesystem mounted.");
            }
        } else {
            crate::kprintln!("Usage: create <filename>");
        }
    }

    fn handle_edit(&self, intent: Intent) {
        // Security Check: Requires System Capability
        if !self.has_capability(crate::kernel::capability::CapabilityType::System) {
            crate::kprintln!("Access Denied: Requires System Capability");
            return;
        }

        use crate::fs::FileSystem;
        if let IntentData::Text(args) = intent.data {
            let mut parts = args.splitn(2, char::is_whitespace);
            let filename = parts.next().unwrap_or("");
            let content = parts.next().unwrap_or("");
            
            if filename.is_empty() {
                crate::kprintln!("Usage: write <filename> <content>");
                return;
            }
            
            if let Some(fs) = crate::fs::get().as_mut() {
                if let Err(e) = fs.write_file(filename, content.as_bytes()) {
                    crate::kprintln!("Error writing file: {}", e);
                } else {
                    crate::kprintln!("Wrote {} bytes to {}", content.len(), filename);
                }
            } else {
                crate::kprintln!("No filesystem mounted.");
            }
        }
    }

    fn handle_rm(&self, intent: Intent) {
        // Security Check: Requires System Capability
        if !self.has_capability(crate::kernel::capability::CapabilityType::System) {
            crate::kprintln!("Access Denied: Requires System Capability");
            return;
        }

        use crate::fs::FileSystem;
        if let IntentData::Text(filename) = intent.data {
            if let Some(fs) = crate::fs::get().as_mut() {
                if let Err(e) = fs.delete_file(filename.as_str()) {
                    crate::kprintln!("Error deleting file: {}", e);
                } else {
                    crate::kprintln!("Deleted file: {}", filename);
                }
            } else {
                crate::kprintln!("No filesystem mounted.");
            }
        } else {
            crate::kprintln!("Usage: delete <filename>");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

use spin::Mutex;

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

static ENGINE: Mutex<SemanticEngine> = Mutex::new(SemanticEngine::new());

pub fn init() {
    ENGINE.lock().init();
}

pub fn run() -> ! {
    init();
    
    let mut input_buffer = [0u8; 256];
    
    loop {
        crate::kprint!("intent> ");
        let len = crate::drivers::uart::read_line(&mut input_buffer);
        if len == 0 { continue; }
        
        let input = core::str::from_utf8(&input_buffer[..len]).unwrap_or("");
        
        // Lock the engine for the duration of understanding and execution
        let mut engine = ENGINE.lock();
        if let Some(intent) = engine.understand(input) {
            engine.execute(intent);
        } else {
            crate::kprintln!("? The void stares back.");
        }
    }
}
