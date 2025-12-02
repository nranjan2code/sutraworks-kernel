# Real Implementation Plan (AI Edition)

This plan addresses the gaps identified in the "No Sugar Coating" review, but with a focus on **making the AI features real** rather than just renaming them.

## Phase 1: The Bridge (USB Input)
**Goal**: Enable actual typing on the kernel via a USB keyboard/steno machine.
**Current State**: xHCI initializes but does not transfer data. `poll()` is empty.

- [x] **Implement xHCI Command Ring**: Ability to send commands (Enable Slot, Address Device).
- [x] **Implement Event Ring Processing**: Read completion events from the controller.
- [x] **Implement Device Enumeration**:
    - [x] Detect device attachment (Port Status Change).
    - [x] Reset port.
    - [x] Assign Slot ID.
    - [x] Address Device.
- [ ] **Implement Transfer Ring**:
    - [ ] Set up Endpoint 1 (Interrupt IN) for the keyboard.
    - [ ] Queue Transfer TRBs to receive keystrokes.
- [ ] **Connect to HID Parser**: Pass received data to `hid.rs`.

## Phase 2: Real Neural Memory (Vector Space)
**Goal**: Transform `NeuralAllocator` from a tagged list into a true **Vector Database** for semantic retrieval.

- [x] **Implement Vector Embeddings**:
    - [x] Add `embedding: [f32; 64]` to `SemanticBlock`.
    - [x] Implement `CosineSimilarity` trait for vectors.
- [x] **Implement Semantic Retrieval**:
    - [x] Create `retrieve_nearest(query_vector: &[f32]) -> Option<IntentPtr>`.
    - [x] This enables "fuzzy" memory recall based on meaning, not just exact ID.
- [x] **Implement "Associative Memory"**:
    - [x] Allow linking blocks based on vector proximity.

## Phase 3: Real Perception (Computer Vision)
**Goal**: Replace the "Red Blob" detector with a real Computer Vision algorithm running on CPU.

- [x] **Implement Edge Detection**:
    - [x] Implement Sobel Operator or Canny Edge Detector in `vision.rs`.
- [x] **Implement Basic Shape Recognition**:
    - [x] Detect lines/rectangles from edges (Hough Transform lite).
- [ ] **(Optional) Tiny Inference**:
    - [ ] If space permits, implement a tiny fixed-weight neural network (MLP) for digit recognition (MNIST-style) to demonstrate *actual* inference.

## Phase 4: Integration
**Goal**: Connect the pieces.

- [x] **Update `main.rs`**:
    - [x] Remove the UART-based "Steno Loop" hack.
    - [x] Use the real `UsbHid` driver to drive the input loop.
