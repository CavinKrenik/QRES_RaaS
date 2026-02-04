# Heterogeneous Determinism: Q16.16 Cross-Platform Guarantees

## Problem Statement

QRES nodes may run on different hardware architectures (ESP32-C6 RISC-V, ARM Cortex-M, x86 simulators). For ZK proofs to be verifiable across the swarm, all nodes must produce **bit-identical** outputs for the same inputs. IEEE 754 floating-point does not guarantee this across architectures due to:

1. Different rounding modes in FPU implementations
2. Fused multiply-add (FMA) availability
3. Extended precision intermediates (x87 80-bit on x86)
4. Compiler-specific optimizations (-ffast-math, auto-vectorization)

## Solution: Q16.16 Fixed-Point Arithmetic

QRES uses `I16F16` (signed 16.16 fixed-point) from the `fixed` crate for all consensus-critical computations.

### Representation

```
[sign][15-bit integer].[16-bit fraction]
 1 bit    15 bits          16 bits
```

- Range: [-32768.0, 32767.999984741]
- Resolution: 2^-16 = 0.0000152588 (~15 μ precision)
- All arithmetic is integer-based: add/sub/mul/div map to integer operations

### Why Fixed-Point Guarantees Determinism

| Operation | IEEE 754 Float | Q16.16 Fixed |
|-----------|---------------|--------------|
| `a + b` | Depends on rounding mode | Always identical (integer add) |
| `a * b` | FMA vs separate mul+add | Always identical (widening mul + shift) |
| `a / b` | Division algorithm varies | Always identical (widening div) |
| Intermediate precision | x87: 80-bit, SSE: 64-bit | Always 32-bit (or 64-bit intermediate) |

### Consensus-Critical Path

The following operations MUST use Q16.16:

1. **Gene residual computation:** `delta = measured - predicted`
2. **Weight update:** `w_new = w_old + lr * delta`
3. **Norm computation:** `||delta||^2 = sum(delta_i^2)`
4. **Aggregation input:** Node's submitted update vector

The following operations MAY use floating-point:

1. **Sensor reading:** Raw ADC values (not consensus-critical)
2. **Display/logging:** Human-readable output
3. **Energy management:** Battery level computation

### Implementation in QRES

```rust
use fixed::types::I16F16;

pub fn deterministic_weight_update(
    weight: I16F16,
    learning_rate: I16F16,
    residual: I16F16,
) -> I16F16 {
    // This produces identical results on x86, ARM, and RISC-V
    weight + learning_rate * residual
}
```

## Cross-Platform Verification Protocol

### Test Vector Generation

A set of canonical test vectors is defined:

```
Input: weight=0x0001_8000 (1.5), lr=0x0000_0666 (~0.025), residual=0x0000_4000 (0.25)
Expected output: 0x0001_8199 (1.50625...)
```

All platforms must produce the exact same output bytes.

### CI Pipeline

```yaml
test-determinism:
  strategy:
    matrix:
      target: [x86_64, armv7, riscv32]
  steps:
    - cargo test --target ${{ matrix.target }} -- test_deterministic_vectors
```

### Runtime Verification (ZK Audit)

The stochastic audit system (Phase 2.3) verifies determinism at runtime:

1. Auditor generates challenge: "recompute weight update for round R with inputs X"
2. Challenged node produces proof binding the computation to inputs
3. Verifier checks the proof against the expected Q16.16 output

If the challenged node used floating-point instead of fixed-point, the output will differ and the proof fails.

## Known Limitations

### 1. Fixed-Point Overflow

`I16F16` overflows at ~32768. For models with large weights, this is a concern. Mitigation:
- Clamp weights to [-100, 100] range (sufficient for edge regression models)
- Use I32F32 for intermediate accumulations if needed (with deterministic truncation back to I16F16)

### 2. Division Rounding

Fixed-point division truncates toward zero. This differs from IEEE 754 round-to-nearest. All nodes must agree on the rounding convention. The `fixed` crate guarantees truncation, so this is consistent.

### 3. Transcendental Functions

`sin`, `cos`, `exp`, `log` are not available in fixed-point. QRES avoids these in the consensus path. If needed, use CORDIC approximations with a fixed iteration count (deterministic by construction).

## Verification Status

| Platform | Tested | Status |
|----------|--------|--------|
| x86_64 (dev) | Yes | Bit-identical |
| ESP32-C6 (RISC-V) | Yes (target build) | Bit-identical |
| ARM Cortex-M4 | Not yet | Expected identical (same integer ops) |
| WASM (browser sim) | Yes | Bit-identical |

## References

- `fixed` crate: https://docs.rs/fixed/
- INV-6 (Bit-Perfect Compliance Auditable): `docs/security/INVARIANTS.md`
- ZK Stochastic Audit: `crates/qres_core/src/zk_proofs.rs` (StochasticAuditor)
