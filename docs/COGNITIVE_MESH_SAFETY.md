# QRES v20 Cognitive Mesh Safety – Invariant Traceability Matrix

This document ensures every new feature introduced in the Cognitive Mesh Evolution (v20) maintains compliance with all six security invariants (INV-1 through INV-6).

---

## Phase 0: Integration & Safety Lock

| Component | Feature | INV-1 | INV-2 | INV-3 | INV-4 | INV-5 | INV-6 | Notes |
|-----------|---------|-------|-------|-------|-------|-------|-------|-------|
| Safety Matrix | This document | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | Traceability for all phases |
| Regression Tests | `tests/invariant_regression.rs` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | Runs after every phase |

---

## Phase 1: Viral Protocol & Asynchronous SGD

| Component | Feature | INV-1 | INV-2 | INV-3 | INV-4 | INV-5 | INV-6 | Safety Mechanism |
|-----------|---------|-------|-------|-------|-------|-------|-------|------------------|
| `packet.rs` | `GhostUpdate::cure_threshold()` | ✓ | - | - | - | ✓ | ✓ | Threshold respects existing reputation bounds; no influence amplification |
| `packet.rs` | `infect()` epidemic gossip | ✓ | ✓ | ✓ | - | ✓ | ✓ | **Energy guard:** Never gossip if `EnergyPool < 15%`; respects existing rate limits |
| `packet.rs` | High-priority cure gossip | ✓ | - | - | - | ✓ | - | Priority queue bounded by reputation; no bypass of rate-limiter |
| Simulation | Straggler scenario testing | - | - | - | - | ✓ | - | Validates viral spread doesn't cause brownouts |

**Critical Invariant Checks:**
- **INV-1:** Cure threshold based on `residual_error` and `accuracy_delta` does NOT allow low-reputation nodes to amplify influence
- **INV-5:** Energy guard prevents epidemic gossip from depleting batteries → no brownouts despite faster propagation
- **INV-6:** All cure metrics use Q16.16 fixed-point (no floating-point drift)

---

## Phase 2: Multimodal SNN & Cross-Correlation Engine

| Component | Feature | INV-1 | INV-2 | INV-3 | INV-4 | INV-5 | INV-6 | Safety Mechanism |
|-----------|---------|-------|-------|-------|-------|-------|-------|------------------|
| `multimodal.rs` | Temporal Attention-Guided Adaptive Fusion | ✓ | - | - | - | ✓ | ✓ | Single-pass attention; no exponential energy cost |
| `multimodal.rs` | Cross-modality "surprise" bias | ✓ | - | - | - | - | ✓ | Additive bias bounded by `Bfp16` range; no overflow |
| `multimodal.rs` | Per-modality learning-rate scaling | ✓ | - | - | - | ✓ | ✓ | Prevents imbalance from causing runaway updates |
| Simulation | Multi-modal energy budget test | - | - | - | - | ✓ | - | Cross-modal fusion ≤ 5% energy increase (hard cap) |

**Critical Invariant Checks:**
- **INV-1:** Temporal attention weighted by reputation → low-reputation nodes cannot hijack cross-modal predictions
- **INV-5:** Energy profiling ensures attention mechanism does NOT increase draw beyond 5% threshold
- **INV-6:** All attention weights and surprise signals use `Bfp16Vec` (no floating-point)

---

## Phase 3: Sentinel Simulation – Virtual Dark-Space Smart City

| Component | Feature | INV-1 | INV-2 | INV-3 | INV-4 | INV-5 | INV-6 | Safety Mechanism |
|-----------|---------|-------|-------|-------|-------|-------|-------|------------------|
| `qres_sim` | 4-zone topology (streetlights, transit, water, energy) | - | ✓ | ✓ | - | - | - | Zone isolation limits Sybil attack surface |
| `qres_sim` | Gossip bridge eligibility (R ≥ 0.8) | ✓ | ✓ | ✓ | ✓ | - | - | High-reputation requirement for inter-zone communication |
| `qres_sim` | `RegimeDetector` entropy-based Calm↔Storm | - | - | - | ✓ | ✓ | - | Local entropy spikes require quorum confirmation |
| `qres_sim` | Lamarckian resumption (non-volatile `GeneStorage`) | - | - | - | - | - | ✓ | 100% weight recovery post-blackout; bit-perfect restore |
| Testing | Coordinated slander attack (1 zone + bridge failure) | ✓ | ✓ | ✓ | - | - | - | Median `PeerEval` + bucketed reputation contains damage |

**Critical Invariant Checks:**
- **INV-2, INV-3:** Zone-based Sybil attack (all fake nodes in one zone) → isolated by bridge eligibility; cannot spread to other zones
- **INV-4:** `RegimeDetector` must verify Storm trigger requires ≥3 nodes with R > 0.8 (quorum gate)
- **INV-6:** Lamarckian recovery must restore Q16.16 weights bit-perfectly (ZK audit post-recovery)

---

## Phase 4: Hardware-Abstracted Security (TEE Prep)

| Component | Feature | INV-1 | INV-2 | INV-3 | INV-4 | INV-5 | INV-6 | Safety Mechanism |
|-----------|---------|-------|-------|-------|-------|-------|-------|------------------|
| `zk_proofs.rs` | `EnclaveGate` trait (`no_std`) | - | - | - | - | ✓ | ✓ | Wraps ZK proofs + energy accounting in single gate |
| `zk_proofs.rs` | Software gate: `report_reputation()` energy check | - | - | - | - | ✓ | - | Fails if `EnergyPool < 10%` (mock PMP/PMA) |
| `zk_proofs.rs` | ZK proof verification wrapper | - | - | - | - | - | ✓ | Future path: hardware-attested determinism |
| Documentation | `SECURITY_ROADMAP.md` Layer 5 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | Documents hardware-attested trust path |
| Documentation | `TEE_MIGRATION_GUIDE.md` | - | - | - | - | - | ✓ | Real silicon checklist (RISC-V/ESP-TEE) |

**Critical Invariant Checks:**
- **INV-5:** Energy gate prevents reputation reporting when battery < 10% → no false Storm triggers during brownout
- **INV-6:** Software `EnclaveGate` prepares API for real TEE hardware → determinism enforced at silicon level

---

## Unified Gauntlet Extension – 200-Round Stress Test

| Test Scenario | INV-1 | INV-2 | INV-3 | INV-4 | INV-5 | INV-6 | Pass Criteria |
|---------------|-------|-------|-------|-------|-------|-------|---------------|
| 35% Byzantine (slander + farming + bridge failure) | ✓ | ✓ | ✓ | - | - | - | Drift ≤ 5% |
| 25% stragglers + intermittent power | - | - | - | - | ✓ | - | 0 brownouts |
| Multi-modal pollution/traffic scenario | ✓ | - | - | - | ✓ | ✓ | Cross-modal prediction works |
| Power-failure + Lamarckian recovery @ round 150 | - | - | - | - | - | ✓ | 100% weight recovery |
| **Combined stress (all scenarios)** | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | All 6 invariants satisfied |

**Gauntlet Rejection Rules:**
- If ANY invariant fails → reject the entire phase
- If drift > 5% → rollback feature
- If brownouts > 0 → energy budget exceeded; redesign required
- If Lamarckian recovery < 100% → INV-6 violation; determinism broken

---

## Implementation Checklist

Before merging any phase:

- [ ] All relevant invariants have ✓ in traceability matrix
- [ ] `tests/invariant_regression.rs` passes for all 6 invariants
- [ ] Gauntlet harness extended with phase-specific stress test
- [ ] No floating-point introduced (INV-6 compliance)
- [ ] Energy profiling confirms no brownouts (INV-5 compliance)
- [ ] Documentation updated (this file + relevant ADRs)

---

## Invariant Quick Reference

| ID | Name | One-Liner |
|----|------|-----------|
| INV-1 | Bounded Influence | Low reputation → zero influence (continuous, no cliffs) |
| INV-2 | Sybil Resistance | Adding k Sybils dilutes per-node power (denominator grows) |
| INV-3 | Collusion Graceful | Colluders bounded by combined reputation (no superlinear gain) |
| INV-4 | Regime Gate | Storm requires ≥3 nodes with R > 0.8 (untrusted quorum blocked) |
| INV-5 | No Brownouts | 0 brownouts in 7-day intermittent solar + adversarial noise |
| INV-6 | Bit-Perfect | Q16.16 fixed-point; ZK auditable; platform-independent |

---

## Contact

Questions about invariant compliance? See:
- Full invariant definitions: [`docs/security/INVARIANTS.md`](security/INVARIANTS.md)
- Roadmap: [`docs/COGNITIVE_MESH_ROADMAP.md`](COGNITIVE_MESH_ROADMAP.md)
- Energy budget analysis: [`docs/power/`](power/)
