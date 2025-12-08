pub mod manifest;
pub mod registry;
pub mod linker;
pub mod manager;
pub mod demo;

pub use manager::APP_MANAGER;

/// Initialize the Intent App Framework
pub fn init() {
    crate::kprintln!("[APPS] Initializing Intent App Framework...");
    
    // 1. Register Demo Skills
    demo::register_demo_skills();
    
    // 2. Load Built-in Apps (for verification)
    let hello_manifest = r#"
app_name: "Hello World"
description: "A simple greeting app"
triggers:
  - input: "Say hello"
flow:
  - id: "greet"
    goal: "Log Hello"
    inputs: ["Hello from Intent Kernel!"]
"#;

    // We need a "Log Hello" skill for this to work.
    // Let's rely on the demo skills for now or add a logging skill.
    // The demo has "Identify Person" and "Unlock Door".
    // Let's use logic that works with demo skills for the default app.
    
    let demo_manifest = r#"
app_name: "Smart Entry"
description: "Unlock door on face detection"
triggers:
  - input: "Face detected"
flow:
  - id: "chk_face"
    goal: "Identify Person"
    inputs: ["$trigger.input"]
  
  - id: "act"
    goal: "Unlock Door"
    inputs: ["$chk_face.result"]
"#;

    if let Err(e) = APP_MANAGER.lock().load_from_string(hello_manifest) {
        crate::kprintln!("[APPS] Failed to load hello app: {}", e);
    }

    if let Err(e) = APP_MANAGER.lock().load_from_string(demo_manifest) {
        crate::kprintln!("[APPS] Failed to load built-in app: {}", e);
    }
}
