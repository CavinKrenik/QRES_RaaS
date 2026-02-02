# QRES Release Notes: RaaS (Resource-Aware Agentic Swarm)

**Current Version:** 19.0.1  
**Project Lead:** Cavin Krenik  
**Core Implementation:** `qres_core` (no_std / I16F16)

---

## v19.0.1: "Secure & Safe" (Current Stable)

**Release Date:** February 2, 2026  
**Status:** High-Integrity Hardening Complete. Verified for Edge Deployment.

### Highlights
v19.0.1 marks the transition from experimental P2P learning to a Formally Verified and Cryptographically Secure swarm protocol. This release introduces the TWT Scheduler, ensuring the swarm can survive indefinitely on finite energy budgets.

#### 1. Target Wake Time (TWT) & Power Management
- **Hardware Alignment:** Integrated Wi-Fi 6 TWT logic into the SilenceController.
- **Reputation-Weighted Sleep:** Reliable nodes "earn" longer DeepSleep intervals (4h), while new/untrusted nodes are forced into active duty (30s intervals) to prove integrity.
- **Energy Accounting:** Implemented MockRadio with mWh tracking, verifying an 82% energy saving during Calm regimes.

#### 2. ZK-Transition Proofs
- **Zero-Knowledge Integrity:** Implemented a non-interactive Sigma protocol using Edwards curves.
- **Verification:** Nodes now prove the validity of their weight updates without exposing raw data. This eliminates the "Front-Door" vulnerabilities common in centralized agent platforms.

#### 3. Formal Verification (TLA+)
- **Liveness Proof:** Formally verified the "Mid-Flight Join" protocol using TLA+.
- **Resilience:** Mathematically proven to reach consensus even under 90% packet loss conditions.

---

## v19.0.0: "The Immune System II"

**Release Date:** February 1, 2026

### Highlights
The "Adversarial Hardening" phase. This release introduced protection against "Inlier Bias" and "Precision Collapse."

- **Robust Aggregation:** Replaced Krum with Coordinate-wise Trimmed Mean. This prevents coordinated Byzantine nodes from "steering" the swarm through subtle, inlier bias attacks.
- **BFP-16 (Block Floating Point):** Solved the vanishing gradient problem. By using a shared exponent for weight blocks, QRES maintains f32 dynamic range while keeping i16 storage density.
- **Summary Genes:** Introduced 74-byte onboarding packets, allowing new nodes to join the swarm with >99% bandwidth reduction compared to full history replay.

---

## v18.0.0: "The Neural Swarm Pivot"

**Release Date:** January 16, 2026

### Highlights
The pivotal shift from a compression utility to a Decentralized Neural Swarm.

- **Lamarckian Persistence:** Introduced GeneStorage. Learned strategies now survive power cycles, allowing "instant-on" intelligence for energy-harvesting hardware.
- **Deterministic Core:** Replaced all floating-point paths with Q16.16 Fixed-Point math. This guarantees bit-perfect consensus across heterogeneous hardware (ARM, x86, RISC-V).
- **Swarm Simulator:** Launched the Bevy-based 3D simulation engine for visualizing emergent self-healing behavior in real-time.

---

## Historical Evolution & Hero Metrics

| Version | Milestone | Key Achievement |
|---------|-----------|-----------------|
| v19.0.x | RaaS | TWT Power Management, ZK-Proofs, TLA+ Verification. |
| v18.0.x | Swarm | 10,000 Node Scalability, Lamarckian Gene Persistence. |
| v17.0.x | Federated | Reputation-Weighted Averaging, Singularity Detection. |
| v16.x.x | Immune | Secure Aggregation (X25519 Masking), Differential Privacy. |

### The "Hero" Stats (Verified)
- **Energy Advantage:** 21.9x (SNN vs ANN architecture).
- **Bandwidth Reduction:** 99% vs standard Federated Learning.
- **Memory Overhead:** < 1 KB per node ($O(1)$ amortized growth).
- **Byzantine Tolerance:** Drift < 5% under 30% coordinated attack.

---

## Breaking Changes & Migration

### v19.0.1 Notice
- **Protocol Change:** TWT interval headers are now mandatory in GhostUpdate packets. v18.x nodes will be ignored by v19.x swarms to prevent energy-draining "Legacy Drift."
- **Hardware Requirements:** Physical TWT support requires ESP32-C6 or compatible Wi-Fi 6 hardware. Tier 2 (Pi Zero) will continue using MockRadio emulation.

### Migration Path
1. Update Cargo.toml to point to `qres_core` v19.0.1.
2. Ensure `ReputationTracker` is initialized in your AppState.
3. Re-run `twt_integration_test.rs` to verify local radio timing.

**Status:** v19.0.1 is the current stable reference for the Resource-Aware Agentic Swarm. Ready for edge-case evaluation and hardware-in-the-loop deployment.
