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
- [x] **Implement Transfer Ring**:
    - [x] Control Endpoint (EP0) Transfer Ring implemented.
    - [ ] Set up Endpoint 1 (Interrupt IN) for the keyboard.
    - [ ] Queue Transfer TRBs to receive keystrokes.
- [x] **Connect to HID Parser**: Pass received data to `hid.rs` (Polling loop implemented).

## Phase 2: Hyperdimensional Memory (HDC)
**Goal**: Transform `NeuralAllocator` into a **Hyperdimensional Computing** engine for cognitive algebra.

- [x] **Implement Hypervectors**:
    - [x] Replace `[f32; 64]` with `[u64; 16]` (1024-bit binary).
    - [x] Implement `HammingSimilarity` (XOR + PopCount).
- [x] **Implement Cognitive Algebra**:
    - [x] `bind(A, B)`: XOR binding.
    - [x] `bundle(A, B)`: Majority superposition.
    - [x] `permute(A)`: Cyclic shift.
- [x] **Verify Logic**:
    - [x] Standalone verification script (`tests/verify_hdc.rs`).
    - [x] **Real Random Projection** (LSH Indexing Implemented)
    - [ ] Implement `Matrix` struct for projection weights (1024 x N).
    - [ ] Implement `matmul_sign` for efficient projection.
    - [ ] Store projection matrices in persistent storage (or generate from seed).
- [ ] **Audio Projection**
    - [ ] Implement MFCC feature extractor.
    - [ ] Implement `AudioHypervector` projection.
    - [x] Prove Orthogonality, Binding, and Unbinding properties.

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
