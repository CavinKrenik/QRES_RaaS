# RaaS Manifest: Resource-Aware Decentralized Node Mesh

**Version:** 20.0.0
**Author:** Cavin Krenik
**Reference Implementation:** QRES (`crates/qres_core`)

---

## Thesis

A decentralized node mesh of autonomous agents operating on physical hardware faces constraints that cloud-hosted AI never encounters: finite battery, limited bandwidth, unreliable links, and adversarial neighbors. RaaS (Resource-Aware Agentic Swarm) is the architectural response to these constraints. It defines three pillars that any physically-grounded autonomous swarm must satisfy to survive indefinitely without human intervention.

---

## Pillar 1: Energy-Bounded Agency

> An agent that exhausts its power supply is dead. Survival requires that every decision &mdash; to compute, communicate, or sleep &mdash; is gated by energy accounting.

### Principle

Every operation has a cost. The swarm must track energy consumption deterministically and refuse operations it cannot afford. This is not optimization; it is a hard constraint. A node that enters Storm mode with 8% battery will brown out mid-consensus, corrupting the round for all peers.

### Implementation in QRES

| Component | Module | Mechanism |
|-----------|--------|-----------|
| **EnergyPool** | `resource_management.rs` | Deterministic energy accounting with `spend()` / `can_afford()` gates |
| **EnergyProfile** | `resource_management.rs` | Hardware-calibrated cost tables (ESP32, Pi Zero, theoretical SNN) |
| **TWTScheduler** | `power/twt_scheduler.rs` | Regime-aware radio sleep (4h/10m/30s) with reputation-weighted intervals |
| **MockRadio** | `power/twt_scheduler.rs` | Simulated radio with mWh energy tracking for testing without hardware |
| **SilenceController** | `adaptive/silence_state.rs` | Utility-gated gossip: suppress broadcasts when `utility < cost` |
| **Brownout Prevention** | `resource_management.rs` | `is_critical()` gate forces regime downgrade when energy < 10% |

### Key Metric

**21.9x energy advantage** (SNN vs ANN architecture). ANN swarms collapse below 10% energy capacity; SNN swarms retain >80% capacity at the same energy budget. Verified in simulation (`v19_verification.rs::verification_snn_vs_ann_energy_collapse`).

### Design Rule

```
IF energy_pool.is_critical() AND desired_regime == Storm THEN
    FORCE regime = Calm    // Survive now, contribute later
```

---

## Pillar 2: Verifiable Integrity

> In a decentralized system with no central authority, every claim must be cryptographically verifiable. Trust is earned through proof, not assertion.

### Principle

A node's contribution to the swarm (its weight update) must be verifiable as legitimate without revealing the underlying weights. Sybil attacks (flooding the swarm with fake identities) must be detectable and punishable. The system must converge correctly even when up to 33% of nodes are adversarial.

### Implementation in QRES

| Component | Module | Mechanism |
|-----------|--------|-----------|
| **ZK Transition Proofs** | `zk_proofs.rs` | Non-interactive Sigma protocol over Edwards curves; proves `\|\|weights\|\| < threshold` |
| **Pedersen Commitments** | `zk_proofs.rs` | Homomorphic commitments: `C = v*H + r*G` with Fiat-Shamir transcript (BLAKE3) |
| **Secure Aggregation** | `secure_agg.rs` | Pairwise x25519 ECDH masking with wrapping arithmetic cancellation |
| **ReputationTracker** | `reputation.rs` | Score-based trust (0.0-1.0): +0.02 for valid ZKP, -0.15 for ZKP failure, ban at < 0.2 |
| **Coordinate-wise Trimmed Mean** | `aggregation.rs` | Replaces Krum; resistant to "Inlier Bias" attacks within 1.5 sigma |
| **Differential Privacy** | `privacy.rs` | L2 clipping + Gaussian noise with epsilon-delta accounting |

### Key Metric

**Drift < 5%** under 30% coordinated Byzantine attack. Sybil attackers (50/50 split) are identified and banned within 4 consensus rounds. Verified in `v19_verification.rs::verification_sybil_attack_weighted_trimmed_mean`.

### Design Rule

```
IF peer.reputation < BAN_THRESHOLD THEN
    EXCLUDE from aggregation    // Protect the swarm
IF zk_proof.verify() == false THEN
    peer.reputation -= 0.15     // Punish fraud
```

---

## Pillar 3: Autonomous Triage

> When conditions change, the swarm must reorganize itself without external coordination. Triage is the ability to prioritize: who sleeps, who wakes, who leads.

### Principle

The network environment is not static. Entropy spikes (data distribution shifts), node failures, and adversarial interference create dynamic conditions. The swarm must detect these changes *predictively*, transition between operating modes, and allocate its limited resources to the most critical tasks. This is triage in the medical sense: stabilize the critical, defer the stable, accept the inevitable.

### Implementation in QRES

| Component | Module | Mechanism |
|-----------|--------|-----------|
| **RegimeDetector** | `adaptive/regime_detector.rs` | 3-point moving average entropy + derivative-based PreStorm trigger |
| **Regime State Machine** | `adaptive/regime_detector.rs` | Calm (conserve) / PreStorm (prepare) / Storm (coordinate) |
| **Reputation-Weighted Sleep** | `power/twt_scheduler.rs` | `interval = base * lerp(0.2, 1.0, reputation)` &mdash; reliable nodes earn sleep |
| **Emergency Wake Protocol** | `power/twt_scheduler.rs` | Sentinel nodes broadcast immediate wake to OnDemand peers on Storm |
| **Gossip Batching** | `power/twt_scheduler.rs` | Messages queued during sleep, burst-transmitted on wake |
| **Gene Gossip** | `packet.rs`, `cortex/` | Panicked nodes request cure genes from evolved neighbors |
| **Summary Genes** | `cortex/` | 74-byte onboarding packets; 99.95% bandwidth reduction vs history replay |

### Key Metric

**4-tick early warning** via PreStorm detection. Nodes self-organize into Sentinel/OnDemand/Scheduled roles with reputation-weighted duty cycles. >80% bandwidth savings during Calm regime. Verified in `twt_integration_test.rs::test_24h_swarm_simulation`.

### Design Rule

```
IF entropy_derivative > threshold THEN
    regime = PreStorm           // Prepare before the crisis
    wake_interval = 10 minutes  // Elevate readiness

IF node.reputation > 0.8 THEN
    sleep_interval *= 1.0       // Earned rest
ELSE IF node.reputation < 0.2 THEN
    sleep_interval *= 0.2       // Must prove yourself
```

---

## The Resource-Aware Difference

Most AI agent frameworks assume unlimited compute, reliable networks, and trusted peers. RaaS assumes none of these. The result is a system that:

| Assumption | Cloud AI Agents | RaaS Agents |
|-----------|-----------------|-------------|
| Power supply | Unlimited (grid) | Finite (battery/solar) |
| Network | Reliable, high-bandwidth | Lossy, 56 kbps, MTU-constrained |
| Peers | Trusted | Up to 33% adversarial |
| Coordination | Central server | Fully decentralized gossip |
| Precision | Float64 | Q16.16 fixed-point (deterministic) |
| State sync | Ship full model | Rematerialize from genes |
| Failure mode | Restart process | Survive, adapt, evolve |

---

## Verification Matrix

Each pillar has verified tests in the QRES test suite:

| Pillar | Test | Assertion |
|--------|------|-----------|
| Energy-Bounded | `verification_snn_vs_ann_energy_collapse` | SNN retains >80% capacity where ANN collapses |
| Energy-Bounded | `test_sleep_saves_energy_vs_baseline` | TWT sleep uses < 50% of always-on energy |
| Energy-Bounded | `test_reputation_weighted_sleep_staggering` | High-rep nodes save more energy than low-rep |
| Verifiable | `verification_sybil_attack_weighted_trimmed_mean` | Sybil attackers banned within 4 rounds |
| Verifiable | `test_norm_proof_valid` / `test_malicious_neuron_rejected` | ZK proofs accept honest, reject fraudulent |
| Verifiable | `test_masking_cancellation_3_peers` | Secure aggregation masks cancel correctly |
| Autonomous | `test_24h_swarm_simulation` | >40% sleep ratio over mixed-regime 24h period |
| Autonomous | `test_prestorm_detection` | PreStorm detected before Storm threshold |
| Autonomous | `test_emergency_wake_response_time` | OnDemand nodes wake instantly on Sentinel broadcast |
| Autonomous | `test_calm_to_storm_transition_sequence` | Regime transitions trigger correct interval changes |

---

## Hardware Target Roadmap

| Tier | Platform | Resource Awareness |
|------|----------|-------------------|
| **1** | ESP32-C6 | Native Wi-Fi 6 TWT, hardware energy metering, BLE beacon fallback |
| **2** | Raspberry Pi Zero 2 W | Linux edge, USB power meter calibration, simulated TWT |
| **3** | RISC-V Custom Silicon | Bare-metal no_std, hardware-level energy gating |
| **4** | x86_64 / WASM | Cloud simulation, benchmark harness, development target |

The `qres_core` library compiles identically for all tiers. Resource awareness is injected through `EnergyProfile` constants and the radio abstraction layer.
