QRES Changelog: Resource-Aware Agentic Swarm (RaaS)
All notable changes to the QRES reference implementation are documented here.

[v19.1.0] - 2026-02-02 "Circadian Rhythm"
Added
TWT Integration: Implemented TWTScheduler in qres_core::power for Wi-Fi 6 (802.11ax) Target Wake Time support.

Regime-Aware Intervals: Calm (4h), PreStorm (10m), Storm (30s).

Reputation-Weighted Sleep: Reliable nodes earn longer sleep cycles; low-trust nodes are forced into higher activity.

MockRadio: Simulated radio power states with deterministic mWh energy tracking for no_std environments.

Gossip Batching: Implemented GossipBatchQueue to queue packets during TWT sleep for high-efficiency burst transmission on wake.

Changed
Energy Accounting: Integrated EnergyPool gates into the SilenceController to force regime downgrades during critical power levels (<10%).

[v19.0.1] - 2026-02-02 "Secure & Safe Hardening"
Added
ZK Transition Proofs: Sigma protocol proofs for weight transitions using Edwards curves and Fiat-Shamir transcripts (BLAKE3).

Formal Verification: Added TLA+ specifications for the "Mid-Flight Join" protocol, proving liveness under 90% packet loss.

Fixed
Identity Exposure: Removed all plaintext API keys and login tokens from the P2P layer; replaced with ZK-verified identity claims.

[v19.0.0] - 2026-02-01 "The Immune System II"
Adversarial Hardening
Trimmed Mean Aggregator: Neutralizes "Inlier Bias" attacks (Drift < 0.05% verified).

BFP-16 Precision: Introduced Block Floating Point for gradient headers to solve the "Vanishing Gradient" problem in fixed-point math.

Summary Gene Protocol: Compact 74-byte onboarding state achieves 2,133:1 compression vs history replay.

[v18.0.0] - 2026-01-15 "Lamarckian Persistence"
Added
Neural Swarm Simulator: tools/swarm_sim (Bevy-based) for visualizing 3D emergent behavior and regime transitions.

Persistence Layer: GeneStorage trait allowing learned strategies to survive reboots/power cycles.

Active Neurons: Refactored Predictor into SwarmNeuron to support spiking logic.

Changed
Architecture: Pivot from "Compression Library" to "Resource-Aware Operating System."

Determinism: Replaced all remaining floating-point paths with I16F16 fixed-point math.

[v16.5.0] - 2026-01-14 "The Immune System I"
Added
The Ghost Protocol:

Differential Privacy: Gaussian noise mechanism for local weight clipping.

Secure Aggregation: Pairwise X25519 masking to hide individual node updates from the aggregator.

Reputation Tracking: Score-based trust (0.0-1.0) with automated banning for ZKP failures.

[v16.0.0] - 2026-01-14 "Engineering Hardening"
Changed
Repository Alignment: Consolidated workspace into qres_core (no_std core), qres_daemon (P2P edge), and bindings.

Zero-Copy: Refactored buffer management to eliminate heap allocations in the critical gossip path.

Panic Removal: Strictly eliminated unwrap() and expect() paths in qres_core for safety-critical deployment.

Fixed
Link Explosion: Implemented Deterministic Seed Sync to reduce P2P overhead to 8 KB/day.

[v15.0.0] - 2026-01-08 "Resource Awareness"
Added
Energy-Bounded Agency: Initial implementation of EnergyPool and ResourceProfile for hardware-calibrated cost tracking.

Regime Detection: 3-point moving average entropy thresholds for Calm and Storm mode switching.

[v10.0.0] - 2026-01-04 "Deterministic Foundations"
Changed
Fixed-Point Engine: First full migration of neural weights to Q16.16 i32 format for cross-platform bit-perfect reproducibility.

Workspace Split: Separated core codec from networking daemon.