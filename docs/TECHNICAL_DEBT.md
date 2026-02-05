# Technical Debt Tracking

**Last Updated:** February 5, 2026  
**Version:** v20.0.1

This document tracks technical debt items requiring resolution in future releases. Items are categorized by priority and include migration paths for breaking changes.

---

## Priority 1: Biological Metaphor Deprecation

### Overview

The codebase currently contains **35+ biological metaphors** across multiple modules that require deprecation in favor of systems engineering terminology. These terms remain in the code for backward compatibility while documentation has been updated to use engineering terms.

### Recommendation: Type Alias Migration Path

Use `#[deprecated]` attributes with type aliases to provide a clean, non-breaking migration:

```rust
// Example migration pattern
#[deprecated(since = "20.2.0", note = "Use `ModelPersistence` instead. See docs/TECHNICAL_DEBT.md")]
pub type GeneStorage = ModelPersistence;

#[deprecated(since = "20.2.0", note = "Use `ModelBytecodePacket` instead. See docs/TECHNICAL_DEBT.md")]
pub type Gene = ModelBytecodePacket;
```

This allows external integrations (Python bindings, qres_daemon clients) to migrate gradually without immediate breaking changes.

---

### Category 1: Neural/Cognitive Metaphors (15+ occurrences)

**Location:** `crates/qres_daemon/src/swarm_p2p.rs` (primary), `crates/qres_core/src/lib.rs`

| Current Term | Engineering Replacement | Occurrences | Migration Strategy |
|--------------|------------------------|-------------|-------------------|
| `LivingBrain` | `ActiveModelAggregator` | 5 | Type alias → trait rename (v21.0) |
| `Cortex` | `AggregationCore` | 3 | Module rename |
| `SwarmNeuron` | `MeshNodeProtocol` | 2 | Trait alias |
| `brain_confidence` | `model_confidence` | 2 | Field rename with deprecation |
| `BrainDelta` | `ModelDelta` | 2 | Struct alias |
| `BrainMessage` | `ModelMessage` | 2 | Enum alias |
| `NeuralSwarm` | `AdaptiveMesh` | 1 | Type alias |

**Deprecation Timeline:**
- **v20.2.0 (Q2 2026):** Add `#[deprecated]` attributes with type aliases
- **v21.0.0 (Q3 2026):** Remove aliases, complete migration (BREAKING)

**Risk Assessment:** Medium. `LivingBrain` is used in public API surface exposed to `qres_daemon` REST endpoints and Python bindings.

---

### Category 2: Collective/Colony Metaphors (10+ occurrences)

**Location:** `crates/qres_daemon/src/swarm_p2p.rs`, `crates/qres_core/src/lib.rs`

| Current Term | Engineering Replacement | Occurrences | Migration Strategy |
|--------------|------------------------|-------------|-------------------|
| `Hive` | `NodeMesh` | 4 | Module rename |
| `SwarmStatus` | `MeshStatus` | 2 | Enum alias |
| `gossip` (as metaphor) | `broadcast` / `propagate` | 8 | Function rename (breaking) |
| `QRES_HIVE_TOPIC` | `QRES_MESH_TOPIC` | 1 | Const rename with deprecated alias |
| `QresBehavior` | `QresMeshProtocol` | 1 | Trait alias |

**Deprecation Timeline:**
- **v20.2.0:** Add const aliases for topic strings
- **v21.0.0:** Rename functions (BREAKING)

**Risk Assessment:** Low. Most usage is internal to `qres_daemon`.

---

### Category 3: Viral/Gene Metaphors (5+ occurrences)

**Location:** `crates/qres_core/src/packet.rs`, `crates/qres_daemon/src/swarm_p2p.rs`

| Current Term | Engineering Replacement | Occurrences | Migration Strategy |
|--------------|------------------------|-------------|-------------------|
| `viral` (protocol) | `accelerated_propagation` | 2 | Module doc update only |
| `gene` | `model_bytecode` | 2 | Already replaced in docs |
| `GeneStorage` | `ModelPersistence` | 1 | Type alias (high priority) |
| `infected nodes` | `propagation_active_nodes` | 1 | Comment update |
| `propagation` (biological) | `broadcast` / `dissemination` | Variable | Context-dependent |

**Deprecation Timeline:**
- **v20.2.0:** Add `GeneStorage` → `ModelPersistence` alias
- **v21.0.0:** Complete removal (BREAKING)

**Risk Assessment:** High. `GeneStorage` is used in persistent state serialization; migration requires careful data format versioning.

---

### Category 4: Singularity/Epiphany Metaphors (3+ occurrences)

**Location:** `crates/qres_daemon/src/swarm_p2p.rs` (lines 284-285, 397-647)

| Current Term | Engineering Replacement | Occurrences | Migration Strategy |
|--------------|------------------------|-------------|-------------------|
| `SignedEpiphany` | `SignedModelUpdate` | 17 | Struct alias (critical path) |
| `Epiphany` | `ModelUpdate` | 5 | Struct alias |
| `epiphany_cost` | `model_update_cost` | 2 | Field rename |
| `SINGULARITY ACHIEVED` | `CONSENSUS ACHIEVED` | 1 | Log message update |

**Deprecation Timeline:**
- **v20.2.0:** Add struct aliases for `SignedEpiphany` → `SignedModelUpdate`
- **v21.0.0:** Complete migration (BREAKING)

**Risk Assessment:** **CRITICAL**. `SignedEpiphany` is serialized in gossip packets and stored in persistent state. Migration requires:
1. Dual serialization support (v20.2.0)
2. Forward/backward compatibility during transition
3. Data migration tool for existing deployments

---

## Priority 2: Implementation Gaps

### MockRadio Hardware Validation

**Status:** Simulation-only implementation  
**Target:** Q2 2026  
**Blocker:** Requires ESP32-C6/S3 hardware with Wi-Fi 6 AP supporting TWT

**Current State:**
- `MockRadio` abstraction in `qres_core::power::twt_scheduler` (lines 122-950)
- Energy measurements **theoretical** based on datasheets:
  - Active TX/RX: ~230 mW
  - Idle Listen: ~80 mW
  - TWT Sleep: ~35 mW
- Simulation shows 82% sleep savings over 24h period

**Required Work:**
1. Implement `HardwareRadio` trait for ESP-IDF TWT API
2. Physical validation with oscilloscope/power monitor
3. Measure real-world TWT service period negotiation overhead
4. Validate sleep savings under varying channel conditions

**Migration Path:**
```rust
// Current (v20.0.1)
let scheduler = TwtScheduler::new(MockRadio::default());

// Future (v21.0.0)
#[cfg(feature = "esp-idf")]
let scheduler = TwtScheduler::new(HardwareRadio::new(twt_config));
```

**Risk Assessment:** Medium. May reveal unexpected power consumption in real deployments.

---

### MultimodalFusion TAAF Implementation

**Status:** ✅ **COMPLETE** (Verified February 5, 2026)  
**Location:** `crates/qres_core/tests/multimodal_verification.rs`  
**Test Results:** **12/12 passing** (100% pass rate)

**Implementation State:**
- Core TAAF functions **fully implemented** in `crates/qres_core/src/multimodal.rs`
- Counter-based LR decay implemented (lines 399-445)
- Reputation³ scaling integrated (line 339)
- All verification tests passing:
  1. ✅ `test_deterministic_bit_check`
  2. ✅ `test_energy_gate_check`
  3. ✅ `test_zkp_validation`
  4. ✅ `test_cross_modal_surprise`
  5. ✅ `test_memory_overhead`
  6. ✅ `test_wrapping_arithmetic_safety`
  7. ✅ `test_full_multimodal_workflow`
  8. ✅ `test_counter_based_lr_scaling`
  9. ✅ `test_reputation_weighting`
  10. ✅ `test_influence_cap_under_slander`
  11. ✅ `test_lr_scaling_high_variance`
  12. ✅ `test_event_driven_attention_heap`

**Verification Command:**
```bash
cargo test --package qres_core --test multimodal_verification --features std
# Result: 12 passed; 0 failed
```

**Future Enhancements (v20.2.0):**
- Wire `ReputationTracker::influence_weight_fixed()` for PeerId-based lookups (architectural refinement)
- Current f32 reputation parameter is functionally correct; PeerId integration deferred to avoid API signature changes

**Risk Assessment:** None. Implementation complete and verified.

---

## Priority 3: Documentation Accuracy

### Bandwidth Claim Variance

**Status:** ✅ **RESOLVED** (February 5, 2026)  
**Issue:** README.md claimed "99% reduction" without noting dataset dependence

**Resolution:**
- Updated to show **4.98x-31.8x compression ratio** (dataset-dependent)
- Preserved "~99% reduction" in context of 8 KB/day vs 2.3 GB/day FL baseline comparison
- Added dataset breakdown: SmoothSine 31.8x, Wafer 4.98x, ECG5000 4.98x

**Verification:**
- [README.md](../README.md) lines 10-12, 57, 69
- Cross-referenced with [V20_IMPLEMENTATION_SUMMARY.md](./V20_IMPLEMENTATION_SUMMARY.md) line 71

---

## Maintenance Guidelines

### Adding New Technical Debt Items

1. **Categorize:** Priority 1 (breaking API), Priority 2 (implementation gaps), Priority 3 (documentation)
2. **Quantify:** Include exact file locations, line numbers, occurrence counts
3. **Risk Assess:** Evaluate impact on external integrations (Python bindings, REST API clients)
4. **Timeline:** Provide target version and deprecation path
5. **Link:** Reference related ADRs, issues, or verification reports

### Deprecation Process

1. **v20.x:** Add `#[deprecated]` attributes with clear migration guidance
2. **v20.x+1:** Emit compiler warnings for deprecated usage
3. **v21.0.0:** Remove deprecated items (major version bump)
4. **CHANGELOG:** Document all breaking changes with migration examples

### No-Breaking-Change Rule

**Critical:** All changes in v20.x releases MUST maintain backward compatibility. Breaking changes require:
- Major version bump (v21.0.0)
- Migration guide in CHANGELOG.md
- Dual-path support for at least one minor release

---

## Audit History

| Date | Auditor | Findings | Resolution |
|------|---------|----------|------------|
| 2026-02-05 | Technical Review | 35+ biological metaphors, MockRadio simulation-only, bandwidth claim variance | Added deprecation tracking, updated README accuracy, documented hardware validation timeline |

---

## See Also

- [CHANGELOG.md](../CHANGELOG.md) - Version history and breaking changes
- [CONTRIBUTING.md](./CONTRIBUTING.md) - Development guidelines
- [COGNITIVE_MESH_SAFETY.md](./COGNITIVE_MESH_SAFETY.md) - Safety invariants
- [MULTIMODAL_VERIFICATION_REPORT.md](./MULTIMODAL_VERIFICATION_REPORT.md) - TAAF implementation status
