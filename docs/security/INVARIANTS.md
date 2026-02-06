# QRES Security Invariants

These are must-hold properties. Each serves as an acceptance criterion for the active defense system.

---

## Consensus-Layer Invariants

### INV-1: Bounded Influence

**Formal Statement:**
For any node `i` with reputation `R_i`, the maximum influence on the consensus update in any single round is bounded by:
```
influence_i <= R_i / sum(R_j for j in active_set)
```
As `R_i -> 0`, `influence_i -> 0` continuously (no cliff at ban threshold).

**How to Test:**
1. Create node with `R = 0.01` (near-zero reputation)
2. Submit adversarial update with bias = 100.0
3. Measure drift: must be < `0.01 * 100.0 / sum(R)` ≈ negligible
4. Compare against same attack with `R = 1.0`: drift should differ by ~100x

**What Breaks if Violated:**
A low-reputation node could produce unbounded steering, defeating the purpose of reputation tracking entirely.

---

### INV-2: Sybil Resistance by Economics

**Formal Statement:**
Adding `k` Sybil identities (each starting at `R = 0.5`) does not increase total Byzantine influence by more than the marginal energy cost of maintaining those identities. Specifically:
```
total_influence(f + k Sybils) <= total_influence(f) + k * R_initial / (n + k)
```
The denominator grows with Sybils, diluting their per-node power.

**How to Test:**
1. Baseline: 100 nodes, 25 Byzantine, measure drift
2. Add 25 Sybil identities (total 50 Byzantine out of 125)
3. Each Sybil starts at R=0.5 but fraction is now 40% (above n/3)
4. Verify: drift increase is < 2x despite doubling attacker count (reputation dilution)

**What Breaks if Violated:**
Attackers can amplify influence by spawning cheap identities, making Byzantine tolerance meaningless.

---

### INV-3: Collusion Degradation is Graceful

**Formal Statement:**
For `k` colluding nodes with combined reputation `sum(R_colluders)`, the maximum achievable drift per round is:
```
drift_collusion <= sum(R_colluders) / sum(R_all) * max_per_round_bias
```
Collusion increases required energy (maintaining reputation) but does not provide superlinear control.

**How to Test:**
1. Run 25% Byzantine with independent attacks: measure drift D1
2. Run 25% Byzantine with perfect collusion: measure drift D2
3. Verify: D2 / D1 < 5x (collusion amplifies but is bounded)
4. Verify: D2 decreases as reputation decays (temporal bound)

**What Breaks if Violated:**
Colluding attackers achieve disproportionate control, potentially exceeding the n/3 safety bound in practice.

---

## Energy-Layer Invariants

### INV-4: No Regime Escalation by Untrusted Quorum

**Formal Statement:**
Transition from Calm/PreStorm to Storm requires confirmation by at least `Q_min = 3` nodes with `R_i > 0.8`. Low-reputation nodes' entropy reports alone cannot trigger Storm.

**How to Test:**
1. Inject entropy spikes from 10 nodes with R < 0.3
2. Verify: regime stays Calm (no Storm transition)
3. Add 3 nodes with R > 0.8 confirming the spike
4. Verify: Storm transition occurs
5. Edge case: 2 high-rep + 100 low-rep → still no Storm

**What Breaks if Violated:**
Attackers force the swarm into Storm regime (30s wake), exhausting batteries across the network. This is a denial-of-service via energy depletion.

---

### INV-5: No Brownouts Under Adversarial Noise

**Formal Statement:**
In the "Intermittent Solar" scenario (6 solar hours/day, 10% storm probability), with 25% Byzantine nodes injecting entropy noise, zero brownouts occur over a 7-day simulation.

**How to Test:**
1. Simulate "Intermittent Solar" week with 25% Byzantine noise injection
2. Byzantine nodes submit entropy values of 3.0 (above Storm threshold) every round
3. With regime consensus gate: verify 0 brownouts
4. Without regime consensus gate (ablation): verify brownouts occur (proving the gate's value)

**What Breaks if Violated:**
Adversarial noise can cause real energy exhaustion, creating availability attacks that persist even after Byzantine nodes are banned.

---

## Determinism Invariant

### INV-6: Bit-Perfect Compliance is Auditable

**Formal Statement:**
For any node `i`, given the same input state and round seed, the output gene residual `delta_i` must be bit-identical across all platforms (x86, ARM, RISC-V). Compliance is verifiable via stochastic ZK challenge every 50 rounds.

**How to Test:**
1. Generate deterministic challenge: (round_seed, input_state)
2. Compute expected output using Q16.16 fixed-point arithmetic
3. Challenge random node to produce ZK proof of correct derivation
4. Verify: proof validates against expected output
5. Adversarial test: node returns floating-point result (non-deterministic) → proof fails → reputation penalty applied

**What Breaks if Violated:**
Non-deterministic nodes produce different consensus states on different platforms, breaking the fundamental assumption of gossip-based consensus. Attackers can exploit platform-dependent rounding to diverge the swarm.

---

## Liveness Invariant

### INV-7: Anti-Stalling / Bounded Convergence

**Formal Statement:**
The mesh must achieve consensus within `T_max = 150` rounds even under 20% straggler conditions. Formally:
```
∀ round r: if stragglers(r) ≤ 0.20 * |active_set|
           then consensus_reached(r + 150) = true
```
Failure to reach consensus within `T_max` triggers an automatic fallback to the last verified stable bytecode stored in the Persistent Storage Layer.

**How to Test:**
1. Configure 100-node mesh with 20% intentional stragglers (delayed responses)
2. Inject consensus challenge at round 0
3. Measure rounds until consensus: must be ≤ 150
4. Inject 25% stragglers (above threshold): verify fallback triggers
5. After fallback: verify all nodes resume from Persistent Storage Layer snapshot

**What Breaks if Violated:**
The mesh can stall indefinitely if slow or malicious nodes prevent convergence. Without a bounded liveness guarantee, an attacker with minority control can create a denial-of-service by perpetually delaying consensus without triggering reputation penalties.

**Implementation Notes:**
- Timeout watchdog in `RegimeDetector` tracks rounds since last consensus
- Fallback uses `ModelPersistence::load_gene()` to restore last stable state
- Straggler detection via heartbeat timeout (3× expected round duration)

---

## Testing Matrix

| Invariant | Unit Test | Integration Test | Simulation |
|-----------|-----------|-----------------|------------|
| INV-1 | `test_bounded_influence` | Gauntlet harness | 1000-node sim |
| INV-2 | `test_sybil_dilution` | Gauntlet harness | Sybil injection sim |
| INV-3 | `test_collusion_graceful` | Gauntlet harness | 40% colluding sim |
| INV-4 | `test_regime_gate_untrusted` | Regime sim | Energy attack sim |
| INV-5 | N/A | Energy sim | Intermittent Solar + noise |
| INV-6 | `test_zk_audit_compliance` | Cross-platform | Heterogeneous VM test |
| INV-7 | `test_straggler_convergence` | Straggler injection | 20% straggler sim |
