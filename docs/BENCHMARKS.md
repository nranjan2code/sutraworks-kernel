# Benchmark Suite Documentation

The Intent Kernel includes a comprehensive benchmark suite tailored to its unique architecture. Unlike generic OS benchmarks, this suite measures the performance of semantic computing components.

## Quick Start

```bash
make run   # Benchmarks run automatically at boot
```

Output appears before "INTENT KERNEL READY (USER MODE)".

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────┐
│                    BENCHMARK SUITE                             │
├────────────────────────────────────────────────────────────────┤
│  Layer 1: Intent Engine          (Semantic Processing)        │
│  Layer 2: HDC Memory             (Hyperdimensional Computing) │
│  Layer 3: Perception Pipeline    (Sensor Fusion)              │
│  Layer 4: Multi-Modal Input      (Steno + English + Vision)   │
├────────────────────────────────────────────────────────────────┤
│  Layer 5: Process/Agent          (Task Management)            │
│  Layer 6: Lock/Synchronization   (SMP Primitives)             │
│  Layer 7: Interrupt/Timer        (Real-Time Path)             │
│  Layer 8: I/O/Networking         (Device Layer)               │
├────────────────────────────────────────────────────────────────┤
│  Layer 9: Memory Allocator       (Slab + Buddy)               │
│  Layer 10: Stress Test           (180k Operations)            │
└────────────────────────────────────────────────────────────────┘
```

---

## Benchmark Categories

### 1. Intent Engine (5 benchmarks)

The core differentiator of Intent Kernel—measures semantic intent processing.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| Handler Match | `bench_intent_broadcast()` | ConceptID comparison speed | Direct u64 equality |
| Registry Lookup | `bench_handler_dispatch()` | Hash → handler resolution | FNV-1a * constant |
| Queue Op | `bench_intent_queue()` | Intent struct creation | Stack allocation |
| Hash Latency | `bench_concept_lookup()` | String → ConceptID | FNV-1a hash |
| Security Check | `bench_security_pipeline()` | Privilege verification | Range comparison |

**Design**: Intents are broadcast (1:N) to all handlers, unlike syscall dispatch (1:1).

### 2. Hyperdimensional Computing (7 benchmarks)

Measures the cognitive memory system—unique to Intent Kernel.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| Hamming Similarity | `bench_hamming_similarity()` | 1024-bit similarity | XOR + popcount |
| Bind (XOR) | `bench_hdc_bind()` | A ⊗ B operation | 16× u64 XOR |
| Bundle (Majority) | `bench_hdc_bundle()` | A + B + C voting | Bit counting |
| Permute | `bench_hdc_permute()` | Cyclic rotation | Bit shift |
| HNSW Search | `bench_hnsw_search()` | Nearest neighbor | Graph traversal |
| Neural Alloc | `bench_neural_alloc()` | Semantic block alloc | Slab + HNSW insert |
| LSH Projection | `bench_lsh_projection()` | Feature → HV | Threshold projection |

**Design**: Hypervectors are 1024-bit binary vectors (`[u64; 16]`). Similarity uses Hamming distance (XOR popcount).

**HNSW Algorithm**:
```
1. Random level assignment (geometric distribution)
2. Greedy search from entry point
3. Bidirectional edge insertion
4. Layer-wise neighbor pruning (M=16, M0=32)
```

### 3. Perception Pipeline (4 benchmarks)

Measures the "Perception Cortex"—sensor fusion for perceptual computing.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| Sensor Fusion | `bench_sensor_fusion()` | N:1 detector merge | Position averaging |
| Perceive+Store | `bench_perceive_and_store()` | Detection → Memory | HV alloc + HNSW |
| Visual HV | `bench_visual_hv()` | Edge → Hypervector | Pattern expansion |
| Object→Concept | `bench_object_to_concept()` | Class ID → ConceptID | Lookup table |

**Design**: Multiple sensors (Hailo-8, CPU, Audio) fuse into unified semantic memory.

### 4. Multi-Modal Input (5 benchmarks)

Measures the multi-path input architecture.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| Steno Stroke | `bench_steno_stroke()` | Key → Intent | Direct lookup |
| Multi-Stroke | `bench_multi_stroke()` | Sequence buffering | Circular buffer |
| English Parse | `bench_english_parse()` | Text → Intent | Phrase matching |
| Synonym | `bench_synonym_expand()` | Word normalization | Hash lookup |
| Dictionary | `bench_dictionary_lookup()` | Stroke → Entry | Binary search |

**Design**: Steno is the fastest path (<0.1μs), English uses phrase database (~200 entries).

### 5. Process & Agent (6 benchmarks)

Measures the semantic agent model.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| Kernel Spawn | `bench_agent_spawn_kernel()` | Agent creation | Stack alloc |
| User Spawn | `bench_agent_spawn_user()` | EL0 agent | Page table setup |
| Context Switch | `bench_context_switch_full()` | Full context swap | yield_task() |
| Preemption | `bench_preemption_latency()` | Timer tick | uptime_ms() |
| Fork | `bench_fork()` | Address clone | Page table walk |
| Exec | `bench_exec()` | ELF loading | Magic check |

**Design**: Agents are semantic processes with their own address space and intent handlers.

### 6. Lock & Synchronization (5 benchmarks)

Measures SMP primitives for multi-core operation.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| SpinLock | `bench_spinlock_uncontended()` | Uncontended lock | Test-and-set |
| Contended | `bench_spinlock_contended()` | Lock + work | Acquire/release |
| Atomic CAS | `bench_atomic_cas()` | Compare-and-swap | LDXR/STXR |
| IPI Latency | `bench_ipi_latency()` | Cross-core signal | GIC SGI |
| Deadlock | `bench_deadlock_detect()` | Cycle detection | Tarjan's SCC |

**Design**: SpinLock disables interrupts during critical sections.

### 7. Interrupt & Timer (4 benchmarks)

Measures real-time responsiveness.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| IRQ Check | `bench_irq_latency()` | GIC read path | MMIO read |
| Timer Jitter | `bench_timer_jitter()` | Clock stability | TSC diff |
| GIC Overhead | `bench_gic_overhead()` | Ack/EOI cycle | Register access |
| Syscall | `bench_syscall_roundtrip()` | SVC dispatch | Switch table |

**Design**: Timer uses ARM Generic Timer (62 MHz).

### 8. I/O & Networking (4 benchmarks)

Measures device I/O paths.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| UART | `bench_uart_throughput()` | Serial bytes/s | MMIO write |
| Ethernet TX | `bench_ethernet_tx()` | Packet prep | Header build |
| TCP Checksum | `bench_tcp_checksum()` | RFC 793 sum | One's complement |
| SD Block | `bench_sd_read()` | Block addressing | Sector calc |

**Design**: TCP checksum uses 16-bit one's complement sum with fold.

### 9. Memory Allocator (2 benchmarks)

Measures hybrid allocator performance.

| Benchmark | Function | What It Measures | Algorithm |
|-----------|----------|------------------|-----------|
| Slab | `bench_memory_alloc()` | 8-byte alloc | Slab cache |
| Buddy | `bench_memory_alloc()` | 4KB alloc | Buddy system |

**Design**: Slab for small objects (<4KB), Buddy for pages (≥4KB).

### 10. Extreme Stress Test (1 benchmark)

Validates allocator under load.

| Test | Operations | Description |
|------|------------|-------------|
| Small Alloc | 100,000 | 8-byte slab |
| Vec Ops | 50,000 | Vec creation |
| Page Alloc | 10,000 | 4KB buddy |
| Mixed | 20,000 | Varying sizes |
| **Total** | **180,000** | ~2M ops/sec |

---

## Measurement Methodology

### Cycle Counting
```rust
let start = profiling::rdtsc();  // CNTVCT_EL0
// ... operation ...
let end = profiling::rdtsc();
let cycles = end.wrapping_sub(start);
```

### Timer Frequency
- ARM Generic Timer at **62 MHz**
- 1 cycle ≈ 16.1 ns

### Preventing Optimization
```rust
core::hint::black_box(result);  // Prevents dead code elimination
```

---

## Typical Results

| Category | Key Metric | Typical |
|----------|------------|---------|
| Intent Handler | Match time | 0 cycles |
| HDC Similarity | 1024-bit compare | 0 cycles |
| HNSW Search | Nearest neighbor | ~800 cycles |
| Steno Stroke | Input→Intent | 42 cycles |
| Context Switch | Full swap | 401 cycles |
| SpinLock | Uncontended | 19 cycles |
| TCP Checksum | 64 bytes | 8 cycles |
| Stress Test | Throughput | 2.1M ops/sec |

---

## Source Code

- **Benchmarks**: [`kernel/src/benchmarks.rs`](../kernel/src/benchmarks.rs)
- **Profiling**: [`kernel/src/profiling.rs`](../kernel/src/profiling.rs)
- **HNSW Index**: [`kernel/src/kernel/memory/hnsw.rs`](../kernel/src/kernel/memory/hnsw.rs)

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md) - System design
- [Security](SECURITY.md) - Intent security pipeline
- [Building](BUILDING.md) - How to build and run
