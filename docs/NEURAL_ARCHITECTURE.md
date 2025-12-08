# Neural Architecture

> **Biologically-Inspired Intent Processing for the Intent Kernel**

## Overview

The Intent Kernel implements a **biologically-inspired neural architecture** that processes intents using mechanisms modeled after the brain:

- **Spreading Activation** - Concepts activate related concepts
- **Lateral Inhibition** - Competing handlers suppress each other
- **Temporal Dynamics** - Activations decay, weak signals accumulate
- **Hierarchical Processing** - Raw → Feature → Object → Semantic → Action
- **Predictive Processing** - Predict outcomes, detect surprise
- **Basal Ganglia Model** - Urgency-based action selection

> [!IMPORTANT]
> **Verified Active (Dec 2025)**: The neural subsystem is wired and running. `decay_tick()` fires every 100ms, `propagate_all()` every 50ms, proven via QEMU test output.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         NEURAL INTENT ARCHITECTURE                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input → [Raw] → [Feature] → [Object] → [Semantic] → [Action] → Output     │
│            ↑         ↑          ↑           ↑                               │
│            └─────────┴──────────┴───────────┘                               │
│                    Top-Down Modulation (Goals)                              │
│                                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Temporal   │  │  Feedback   │  │  Attention  │  │  Scheduler  │        │
│  │  Dynamics   │  │   Loops     │  │   Focus     │  │ Integration │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Core Neural Primitives

### Intent with Neural Fields

```rust
pub struct Intent {
    pub concept_id: ConceptID,
    pub confidence: f32,
    pub data: IntentData,
    pub name: &'static str,
    
    // Neural fields
    pub activation: f32,           // Signal strength (decays)
    pub timestamp: u64,            // Creation time
    pub level: IntentLevel,        // Processing stage
    pub source: Option<ConceptID>, // For feedback loops
}
```

### Activation Levels

Concepts have dynamic activation levels (0.0 - 1.0):

- **1.0**: Fully activated (just fired)
- **0.5**: Moderately active
- **0.1**: Threshold for being "active"
- **0.0**: Inactive

Activation naturally decays over time via `decay_tick()`.

### IntentLevel Hierarchy

```rust
pub enum IntentLevel {
    Raw = 0,       // Sensory input (pixels, samples)
    Feature = 1,   // Detected features (edges, phonemes)
    Object = 2,    // Recognized objects (face, word)
    Semantic = 3,  // Meaning (person=friend, word=command)
    Action = 4,    // Motor output (speak, move, display)
}
```

---

## Handler System

### Neural Handler Entry

```rust
pub struct HandlerEntry {
    pub concept_id: ConceptID,
    pub handler: HandlerFn,
    pub priority: u8,
    pub name: &'static str,
    
    // Neural fields
    pub inhibits: [ConceptID; MAX_INHIBITS],  // Lateral inhibition
    pub refractory_ms: u16,                    // Can't fire for N ms
    pub last_fired: u64,                       // For refractory check
}
```

### Handler Results

```rust
pub enum HandlerResult {
    Handled,           // Processed successfully
    NotHandled,        // Did not process
    Error,             // Processing failed
    StopPropagation,   // Stop broadcasting
    
    // Neural results
    Inhibit(ConceptID),     // Suppress another handler
    Modulate(f32),          // Adjust global gain
}
```

### Broadcast Scoping

```rust
pub enum BroadcastScope {
    Local,      // Exact ConceptID match only
    Subsystem,  // Same prefix (first 2 bytes)
    Global,     // All matching + wildcards
}
```

### Conflict Resolution

```rust
pub enum ConflictResolution {
    FirstClaims,        // First handler wins
    HighestPriority,    // Highest priority wins
    HighestActivation,  // Highest activation wins
    Consensus,          // All must agree
}
```

---

## Temporal Dynamics

### Decay Tick

All activations decay over time:

```rust
// Call periodically (e.g., every 100ms)
TEMPORAL_DYNAMICS.lock().tick(timestamp);

// Or use convenience function
decay_tick(timestamp);
```

### Temporal Summation

Weak signals accumulate within a time window:

```rust
// Three weak signals (0.15 each) within 100ms
allocator.temporal_summate(concept, 0.15, t1, 100);  // → activation = 0.15
allocator.temporal_summate(concept, 0.15, t2, 100);  // → activation = 0.30
allocator.temporal_summate(concept, 0.15, t3, 100);  // → activation = 0.45 (threshold!)
```

**Biological Analogy**: Like EPSPs summing at a neuron's soma.

### Predictive Priming

Concepts are pre-activated based on learned sequences:

```rust
// Learn: A → B
allocator.record_sequence(A, B, timestamp, 500);

// Later, activating A primes B
allocator.activate(A, 1.0, timestamp);
allocator.apply_predictive_priming(A, timestamp);

// B is now primed (faster to activate)
assert!(allocator.is_primed(B));
```

---

## Hierarchical Processing

### Layer Architecture

```
Raw → Feature → Object → Semantic → Action
  ↑       ↑        ↑         ↑
  └───────┴────────┴─────────┘
       Top-Down Modulation
```

### HierarchicalProcessor

```rust
pub struct HierarchicalProcessor {
    layers: [LayerBuffer; 5],  // One per IntentLevel
    attention: AttentionFocus,
    goals: GoalState,
}
```

### Attention Focus

Limited-capacity selective attention:

```rust
pub struct AttentionFocus {
    attended: heapless::Vec<ConceptID, 8>,
    weights: heapless::Vec<f32, 8>,
    capacity: usize,
    global_gain: f32,
    suppression: f32,  // For unattended items
}
```

- **Attended items**: Get boosted activation
- **Unattended items**: Get suppressed (30% of normal)
- **Limited capacity**: Only 4-8 items can be attended

### Goal-Based Modulation

```rust
pub struct GoalState {
    goals: heapless::Vec<ConceptID, 4>,
    priorities: heapless::Vec<f32, 4>,
    modulation_strength: f32,
}
```

Goals affect perception:
- Goal-relevant concepts get boosted
- Non-goal concepts get slightly suppressed

---

## Feedback Loops

### Prediction (Efference Copy)

When taking an action, predict its outcome:

```rust
// Predict: opening file will result in "file opened"
predict(ACTION_OPEN, RESULT_OPENED, 0.9, timestamp);
```

### Expectation Matching

Compare predictions against actual input:

```rust
let result = process_input(actual_concept, timestamp);

if result.was_predicted {
    // Expected - no surprise
} else {
    // Unexpected - generate surprise
}
```

### Surprise Detection

```rust
pub enum SurpriseType {
    Unexpected,  // Not predicted
    Omission,    // Predicted but didn't happen
    Mismatch,    // Different from expected
}

pub struct SurpriseDetector {
    cumulative_surprise: f32,
    threshold: f32,
}
```

Surprise signals:
- Boost attention to unexpected events
- Increase priority for processing
- Trigger learning/adaptation

---

## Scheduler Integration

### Intent Requests

```rust
pub struct IntentRequest {
    pub concept_id: ConceptID,
    pub priority: u8,           // Static priority (0-255)
    pub urgency: f32,           // Dynamic urgency (0.0-1.0)
    pub surprise_boost: f32,    // From feedback system
    pub preferred_core: Option<u8>,
}
```

Effective priority = `priority * urgency * surprise_boost`

### Urgency Accumulator (Basal Ganglia Model)

```rust
pub struct UrgencyAccumulator {
    urgencies: heapless::Vec<(ConceptID, f32, u64), 16>,
    threshold: f32,
    dopamine: f32,           // Reward signal
    tonic_inhibition: f32,   // Default = don't act
}
```

**Biological Analogy**:
- **Striatum**: Collects urgency signals
- **GP/SNr**: Tonic inhibition (threshold)
- **Dopamine**: Modulates all urgencies
- **Thalamus**: Releases selected action

### Core Affinity

```rust
pub enum IntentCategory {
    Input = 0,      // Core 0: steno, audio
    Compute = 1,    // Core 1: perception, inference
    Output = 2,     // Core 2: display, speech
    Background = 3, // Any core
}
```

Categories are assigned based on ConceptID high byte.

### Graceful Degradation

```rust
pub enum LoadLevel {
    Normal,    // Full processing
    High,      // Skip background, min priority 64
    Critical,  // Reduce perception, min priority 128
}
```

| Load | Effect |
|------|--------|
| < 80% | Normal processing |
| 80-95% | Skip background tasks |
| > 95% | Aggressive degradation |

---

## Semantic Memory (NeuralAllocator)

### SemanticBlock

```rust
pub struct SemanticBlock {
    pub concept_id: ConceptID,
    pub access_count: u64,
    pub size: usize,
    
    // Neural fields
    pub activation: f32,
    pub last_accessed: u64,
    pub associations: [ConceptID; 8],
    pub link_strengths: [f32; 8],
}
```

### Spreading Activation

```rust
// Activate a concept
let activated_associates = allocator.activate(concept_id, 1.0, timestamp);

// Associates are automatically activated (with decay)
```

### Hebbian Learning

"Neurons that fire together, wire together":

```rust
// Strengthen association between A and B
allocator.associate(A, B, 0.1);
```

---

## Global Instances

| Instance | Description |
|----------|-------------|
| `TEMPORAL_DYNAMICS` | Decay and temporal processing |
| `HIERARCHICAL_PROCESSOR` | Layer propagation and attention |
| `FEEDBACK_PROCESSOR` | Prediction and surprise |
| `NEURAL_SCHEDULER` | Intent scheduling |
| `NEURAL_ALLOCATOR` | Semantic memory |

---

## Convenience Functions

### Temporal

```rust
decay_tick(timestamp);
process_intent_activation(concept, strength, timestamp);
summate(concept, strength, timestamp);
is_primed(concept);
```

### Hierarchical

```rust
input_intent(&intent);
propagate_all();
attend(concept);
set_goal(goal, priority);
get_actions();
```

### Feedback

```rust
predict(source, predicted, confidence, timestamp);
expect(concept, strength, timestamp);
process_input(concept, timestamp);
surprise_level();
priority_boost();
```

### Scheduling

```rust
submit_intent(request);
next_intent();
next_intent_for_core(core_id);
update_load(load);
scheduler_tick(timestamp);
```

---

## Neural Features Summary

| Feature | Module | Description |
|---------|--------|-------------|
| Activation Levels | `neural.rs` | Dynamic concept strength |
| Spreading Activation | `neural.rs` | Activate associates |
| Lateral Inhibition | `handlers.rs` | Suppress competitors |
| Refractory Periods | `handlers.rs` | Post-firing delay |
| Hebbian Learning | `neural.rs` | "Fire together, wire together" |
| Broadcast Scoping | `handlers.rs` | Local/Subsystem/Global |
| Conflict Resolution | `handlers.rs` | Winner-take-all |
| Decay Tick | `temporal.rs` | Automatic decay |
| Temporal Summation | `temporal.rs` | Weak signals accumulate |
| Sequence Learning | `temporal.rs` | A→B patterns |
| Predictive Priming | `temporal.rs` | Pre-activate expected |
| Layer Propagation | `hierarchy.rs` | Raw→Action |
| Attention Focus | `hierarchy.rs` | Selective enhancement |
| Goal Modulation | `hierarchy.rs` | Top-down influence |
| Efference Copy | `feedback.rs` | Predict outcomes |
| Expectation Matching | `feedback.rs` | Compare vs actual |
| Surprise Detection | `feedback.rs` | Flag unexpected |
| Priority Boost | `feedback.rs` | Surprise → priority |
| Intent Preemption | `scheduling.rs` | Urgency preemption |
| Urgency Accumulation | `scheduling.rs` | Basal ganglia model |
| Core Affinity | `scheduling.rs` | Intent → Core |
| Graceful Degradation | `scheduling.rs` | Load-based throttling |

---

## Module Structure

```
kernel/src/intent/
├── mod.rs          # Core types, Intent, ConceptID
├── handlers.rs     # Broadcast, inhibition, scoping
├── queue.rs        # Priority queue
├── security.rs     # Intent security
├── temporal.rs     # Decay, summation, priming
├── hierarchy.rs    # Layers, attention, goals
├── feedback.rs     # Prediction, expectation, surprise
└── scheduling.rs   # Priority, urgency, degradation
```

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Intent activation | O(1) | Simple field update |
| Spreading activation | O(k) | k = number of associates |
| Decay tick | O(n) | n = active concepts |
| Handler broadcast | O(m) | m = registered handlers |
| Hierarchical propagation | O(layers × intents) | 5 layers max |
| Urgency selection | O(u) | u = pending urgencies |

### The Cost of Cognition

Implementing biological realism comes with a computational cost. Our benchmarks reveal a **~100x latency gap** between raw intent dispatch and full neural processing:
- **Reflex (Dispatch)**: ~54 cycles (Spinal cord speed)
- **Thought (Neural)**: ~5,989 cycles (Cortical speed)

This is a **feature**, not a bug. It means the system can handle high-frequency inputs (steno typing at 300 WPM) without getting bogged down by deep thought, while still applying complex logic (inhibition, prediction) when necessary. We trade cycles for intelligence, but only where it matters.

---

## Design Philosophy

1. **Biologically Plausible**: Mechanisms inspired by real neural systems
2. **Real-Time Capable**: All operations bounded, no unbounded loops
3. **Zero Allocation**: Uses heapless collections where possible
4. **Modular**: Each neural feature is independent but composable
5. **Observable**: Statistics available for all subsystems

---

---

## System 2: LLM Cognitive Engine

While the Intent Engine (System 1) handles fast, reflex-like interactions (~54 cycles), the **LLM Engine (System 2)** handles deep semantic reasoning, complex language generation, and world knowledge (~370,000+ cycles).

### Architecture

The LLM Engine is implemented as a standalone kernel module (`kernel/src/llm`) that integrates with the filesystem and scheduler.

#### Weight Loading & Memory Management
The `llm::loader` module manages the loading of large model weights (e.g., Llama 2) from the SD card.

- **Format**: Custom binary format (`.bin`) optimized for `no_std` environments:
  - Header: 7 x `u32` (Config: dim, layers, heads, etc.)
  - Body: Flat `f32` array of all weights (planar layout).
- **Ownership**: 
  - `OwnedWeights`: Owns the backing `Vec<f32>` buffer designated for the model.
  - `Weights`: A lightweight struct of slices (`&'a [f32]`) referencing the owned data, used during inference.
- **Fail-Safe Fallback**: If `model.bin` is missing or corrupt, the loader initializes a "Dummy Model" (small buffer) to ensure the kernel always boots and the inference pipeline remains testable, preventing hard panics.

### Inference Pipeline
The inference engine implements a standard Transformer forward pass (Llama 2 architecture) in pure Rust (`no_std`):

1. **Tokenization**: Maps English text to tokens (Vocabulary index).
2. **Embedding**: Retrieval of learned vector representations.
3. **Attention**: Multi-Head Attention (MHA) with Grouped Query Attention (GQA).
4. **Feed-Forward**: SwiGLU activations.
5. **Sampling**: Logits -> Probability -> Token Selection.

### System 1 vs System 2 Integration
- **System 1 (Intent)**: Recognizes "ask", "query", "explain".
- **System 2 (LLM)**: Invoked when System 1 detects a need for complex processing.
- **Latency**: The 100x+ latency gap is bridged by the Intent Framework's async capabilities, allowing the kernel to remain responsive (handling interrupts/steno) while the LLM "thinks" in the background.

---

## Future Directions

- **Spiking Neural Network** mode for extreme efficiency
- **Neuromorphic Hardware** integration (Loihi, TrueNorth)
- **Continuous Learning** with online Hebbian updates
- **Neuromodulator System** (dopamine, serotonin, norepinephrine)
