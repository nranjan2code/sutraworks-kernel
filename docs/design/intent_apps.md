# Intent-Native Applications: Building at the Speed of Thought

## The Philosophy
In the Intent Kernel, **code is an implementation detail**. The user should never have to think about loops, variables, or memory management to solve a problem. They should only express their **Intent**.

An "Application" in this OS is not a binary. It is a **Manifest of Intents**—a declarative map that connects a user's desires (Concepts) to the system's capabilities (Skills).

## The Core Primitives

### 1. The Intent Manifest (`.intent`)
Instead of compiling code, the user (or an AI agent) generates an `Intent Manifest`. This is a high-level graph defining the flow of information between concepts.

**Structure:**
```yaml
app_name: "Calorie Tracker"
triggers:
  - input: "I ate a [Food: String]"
    intent: "LOG_FOOD"
flow:
  - on: "LOG_FOOD"
    steps:
      1. resolve: "Food Database" (Capability)
      2. action: "Lookup Calories" (Input: Food)
      3. resolve: "User Journal" (Capability)
      4. action: "Append Entry" (Input: Food, Calories, Time)
      5. feedback: "Show Notification" (Text: "Logged {Calories} kcal")
```

**Kernel Implementation (`intent::manifest`):**
```rust
pub struct IntentManifest {
    pub app_name: String,
    pub triggers: Vec<Trigger>,
    pub flow: Vec<FlowStep>,
}

pub struct Trigger {
    pub input_pattern: String,
    pub intent: ConceptID,
}

pub struct FlowStep {
    pub on_intent: ConceptID,
    pub action: String,
    pub parameters: Vec<String>,
}
```

### 2. The Semantic Linker (The "Compiler")
Traditional OSes link symbols (function names) at compile time. The Intent Kernel links **Semantics** at runtime.

*   **Mechanism**: The Linker uses Hyperdimensional Computing (HDC) to find the best match for a requested capability.
*   **Example**: If the manifest asks for "Food Database", the Linker searches the local vector space for installed skills. It might find `NutritionIX_API_Skill` (Similarity: 0.95) or a local `CSV_Lookup_Skill` (Similarity: 0.88).
*   **Benefit**: Apps are **polymorphic**. The same "Calorie Tracker" manifest works whether you have a cloud API plugin or a local file plugin installed. The "code" changes, but the intent remains valid.

**Kernel Implementation (`intent::linker`):**
```rust
pub trait SemanticLinker {
    /// Find a capability matching the description using vector similarity
    fn find_capability(&self, description: &str) -> Option<String>;
}
```

### 3. The Skill Registry
Developers don't build "Apps"; they build **Skills** (Atomic Capabilities).
*   **Skill**: A small, focused WASM module or Kernel Driver that exposes a set of typed Intents.
    *   *Example*: `CameraSkill` exposes `CAPTURE_IMAGE`, `SCAN_QR`.
    *   *Example*: `SpotifySkill` exposes `PLAY_MUSIC`, `SEARCH_SONG`.
*   These skills are registered in the global Semantic Space.

## Technical Specification

### Skill Interface
Skills are the atomic building blocks. They must implement a standard interface to be discoverable by the Linker.

```rust
pub trait Skill {
    /// The semantic description of what this skill does (for HDC embedding)
    fn description(&self) -> &'static str;
    
    /// The set of intents this skill can handle
    fn supported_intents(&self) -> &[ConceptID];
    
    /// Execute an intent
    fn execute(&self, intent: ConceptID, params: &[Param]) -> Result<Output, Error>;
}
```

### Data Typing & Validation
Data flow between steps is typed to ensure compatibility.
- **Primitive Types**: String, Number, Boolean.
- **Semantic Types**: `ConceptID` (references to other concepts).
- **Validation**: The Linker verifies that the Output type of Step N matches the Input type of Step N+1 during the linking phase.

### Error Handling Policy
- **Retry**: Transient errors (network timeout) trigger automatic retries with exponential backoff.
- **Fallback**: If a primary skill fails (e.g., Cloud Database offline), the Linker can dynamically re-route to a secondary skill (e.g., Local Cache) if one exists with sufficient semantic similarity.
- **User Intervention**: Critical failures bubble up to the user via the Semantic Visual Interface (SVI).

### Security Model
- **Least Privilege**: Manifests request capabilities by description. The user must approve the *class* of capability (e.g., "Allow access to Health Data?") once.
- **Sandboxing**: WASM skills run in isolated memory spaces.
- **Audit Trail**: All intent executions are logged to the Intent History, providing a transparent record of what the system did and why.

## The User Experience: "Programming" without Code

### Mode A: Conversational Construction
The user simply tells the OS what they want.
> **User**: "I want to track what I eat. When I say 'I ate an apple', look up its calories and save it to a list."

**The OS Agent:**
1.  Parses the sentence.
2.  Identifies triggers: "I ate [Food]".
3.  Identifies actions: "Look up calories", "Save to list".
4.  **Generates the Manifest** automatically.
5.  **Links** it to available skills (e.g., `NutritionSkill`, `FileSkill`).
6.  **Deploys** it instantly.

### Mode B: Visual Mind Mapping (SVI)
The user opens the Semantic Visual Interface (SVI).
1.  They drag a "Voice Input" node onto the canvas.
2.  They drag a "Database" node.
3.  They draw a line between them.
4.  The OS asks: "What is the relationship?"
5.  User types/says: "Store the text."
6.  The OS creates the semantic link.

## Example: "The 3-Sentence App"

**Goal**: A localized "Baby Monitor" that alerts me if it hears crying.

**User Intent**:
1.  "Listen to the microphone continuously."
2.  "If you hear a sound like 'Crying', turn the screen Red."
3.  "Otherwise, keep the screen Green."

**System Execution**:
1.  **Link**: Connects `AudioInput` -> `AudioClassifier`.
2.  **Configure**: Sets `AudioClassifier` target to vector `[Crying]`.
3.  **Link**: Connects `Classifier.Match` -> `Screen.SetColor(Red)`.
4.  **Link**: Connects `Classifier.NoMatch` -> `Screen.SetColor(Green)`.

**Result**: A fully functional, real-time embedded application created in seconds, running on the bare metal Intent Kernel.
