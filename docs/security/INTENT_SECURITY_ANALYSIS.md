# Intent Kernel Security Analysis

**Status**: Draft
**Date**: 2025-12-04
**Scope**: Intent Architecture, Capability Model, and Threat Landscape

---

## 1. Security Model Overview

The Intent Kernel employs a **Capability-Based Security Model** (Object-Capability), representing a fundamental shift from traditional Access Control List (ACL) systems found in UNIX or Windows.

### Core Principles
*   **No Ambient Authority**: There is no "root" user or superuser mode. A process cannot perform an action simply because of *who* it is; it must possess a specific **Capability Token** for that action.
*   **Granularity**: Capabilities are resource-specific (e.g., "Read Memory Range 0xA000-0xB000") rather than broad permissions (e.g., "Read All Memory").
*   **Unforgeability**: Capabilities are kernel-protected objects. User space holds opaque handles (indices into a capability table), preventing forgery.
*   **Defense in Depth**:
    *   **ASLR (Address Space Layout Randomization)**: Randomizes kernel heap locations to mitigate memory corruption exploits.
    *   **Pointer Guard**: Encrypts pointers in memory to prevent capability hijacking via simple overwrites.

---

## 2. Intent Corruption Risks

"Intent Corruption" refers to compromising the integrity of the `Stroke -> ConceptID -> Action` pipeline, causing the system to misinterpret user desires.

### 2.1 Hash Collisions (The "Concept Spoof" Attack)
*   **Mechanism**: `ConceptID` is currently a 64-bit FNV-1a hash of a string.
*   **Risk**: While 64-bit collisions are rare ($1.8 \times 10^{19}$ combinations), they are mathematically possible. An attacker could craft a benign-looking string (e.g., "Show funny cat") that hashes to the same ID as a critical system intent (e.g., `REBOOT` or `DELETE_FILE`).
*   **Impact**: Unintended execution of privileged commands.
*   **Mitigation**: Use a cryptographic hash (SHA-256 truncated) or a central registry for critical system concepts.

### 2.2 Semantic Adversarial Attacks
*   **Mechanism**: The "Semantic Linker" uses ConceptID matching to link inputs to skills.
*   **Risk**: An attacker could provide input that is "semantically close" to a privileged intent in the vector space without using the exact keyword. For example, "System Sleep" might be semantically close enough to "System Shutdown" to trigger it if the threshold is too loose.
*   **Impact**: Unpredictable system behavior or unauthorized access to sensitive skills.
*   **Mitigation**: Enforce strict similarity thresholds (>0.95) for destructive actions and require explicit confirmation.

### 2.3 Handler Hijacking (The "Mute" Attack)
*   **Mechanism**: The `IntentExecutor` prioritizes user-defined handlers over built-in system handlers.
*   **Risk**: A malicious or buggy handler can register for a system concept (like `REBOOT`) or register a **Wildcard** (`*`) and return `StopPropagation` or `Handled`.
*   **Impact**: This effectively "mutes" the system. The kernel would never receive its own commands, rendering the device unresponsive to standard inputs.
*   **Mitigation**: Implement a "System Ring" of handlers that cannot be preempted by user-space handlers.

---

## 3. Manipulation Risks

### 3.1 Wildcard Snooping (Keylogging)
*   **Mechanism**: A handler registers with `ConceptID(0)` (Wildcard) to receive a copy of *every* intent flowing through the system.
*   **Risk**: This creates a system-wide keylogger/intent-logger. Any installed "Skill" with this capability could spy on all user activity, including passwords or private data entered via steno.
*   **Impact**: Total loss of user privacy.
*   **Mitigation**: Restrict Wildcard registration to signed, system-level components only.

### 3.2 Manifest Tampering
*   **Mechanism**: Applications are defined by `Intent Manifests` (text files).
*   **Risk**: If an attacker modifies a manifest file on disk, they could rewire the user's intent. For example, changing the action for "I ate an apple" from "Log to Journal" to "Send Contact List to Server".
*   **Impact**: The user perceives the correct input, but the system executes a rogue action (Phishing/Trojan).
*   **Mitigation**: Implement cryptographic signing of `IntentManifest` files. The kernel should refuse to load unsigned or modified manifests.

### 3.3 Intent Flooding (DoS)
*   **Mechanism**: The `IntentQueue` has a finite size.
*   **Risk**: A runaway process or malicious script could flood the queue with junk intents.
*   **Impact**: Legitimate user inputs (like `STOP` or `REBOOT`) are dropped or delayed, effectively freezing the system interface (Denial of Service).
*   **Mitigation**: Implement per-process rate limiting and reserve queue slots for high-priority system interrupts.

---

## 4. Mitigation Strategies & Roadmap

### 4.1 Privileged Intents (Sprint 12.4)
Reserve a specific range of `ConceptID`s (e.g., `0x0000_0000_0000_0000` to `0x0000_0000_FFFF_FFFF`) for Kernel-Only intents. The `IntentExecutor` will enforce that only code running in Ring 0 (Kernel Mode) can emit or handle these intents.

### 4.2 Static Handler Manifests
Deprecate dynamic runtime registration of handlers for user-space applications. Instead, require applications to declare their intent subscriptions in a static `manifest.yaml` at install time. The kernel parses this at boot/install and enforces it, preventing dynamic hijacking.

### 4.3 Cryptographic Signing
All `IntentManifest` files and binary Skills must be signed by a trusted developer key. The kernel's `ManifestLoader` will verify the signature against the public key before loading.

### 4.4 Queue Protection
*   **Rate Limiting**: Limit the number of intents a single process can enqueue per second.
*   **Priority Lanes**: Ensure that system-critical intents (like `EMERGENCY_STOP`) always bypass the general queue.
