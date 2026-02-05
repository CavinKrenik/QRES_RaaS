# Reputation Privacy and Anti-Slander Mechanisms

## Problem: Slander Attacks

In the current QRES design, each node evaluates its peers via PeerEval and these evaluations influence reputation. A Byzantine node can exploit this by:

1. **Direct Slander:** Assigning PeerEval = 0 to targeted honest nodes
2. **Coordinated Slander:** Multiple colluding nodes gang up on a single honest node
3. **Selective Slander:** Targeting high-reputation honest nodes to reduce their influence

### Impact Analysis

With default parameters (DRIFT_PENALTY = 0.08):
- A single slander event reduces target's reputation by 0.08
- An honest node starting at R=0.8 would need 8 consecutive slander rounds to reach ban threshold (0.2)
- With $f$ colluding slanderers, the target is effectively penalized $f$ times per round

## Defense: Median PeerEval Aggregation

### Current Design (Vulnerable)

```
R_i(t+1) = (1 - gamma) * R_i(t) + gamma * mean(PeerEval_j(i) for j in swarm)
```

A single Byzantine evaluator can shift the mean by up to $1/n$.

### Proposed Design (Slander-Resistant)

```
R_i(t+1) = (1 - gamma) * R_i(t) + gamma * median(PeerEval_j(i) for j in swarm)
```

**Theorem:** With $f < n/3$ Byzantine evaluators, the median PeerEval is always controlled by honest evaluators.

**Proof:** The median requires $> n/2$ values to be on one side. Since $f < n/3$, the $n - f > 2n/3$ honest evaluators form the majority, and the median falls within the range of honest evaluations.

### Reputation-Weighted Median

For additional robustness, weight evaluators by their own reputation:

```
weighted_median(
    values = [PeerEval_j(i) for j in swarm],
    weights = [R_j for j in swarm]
)
```

This further reduces Byzantine influence because slanderers accumulate low reputation over time (their own bad updates are detected), reducing their weight in the evaluation.

## Slander Detection Heuristic

Even with median PeerEval, it is useful to detect slanderers for logging and forensic purposes.

### Detection Rule

For each evaluator $j$ and evaluated node $i$:

```
slander_score(j, i) = count(rounds where PeerEval_j(i) < 0.3 AND median(PeerEval(i)) > 0.7)
```

If `slander_score(j, i) > 5` over the last 20 rounds, flag node $j$ as a potential slanderer targeting node $i$.

### Action on Detection

1. Log the slander event with full evaluation context
2. Do NOT automatically penalize the suspected slanderer (to avoid meta-slander attacks)
3. Include slander reports in the periodic health report for operator review

## Privacy Considerations

### Reputation Visibility

**Current:** Reputation scores are visible to all nodes (needed for weighted aggregation).

**Risk:** A Byzantine node can see which honest nodes have the highest reputation and target them.

**Mitigation options:**

1. **Bucketed Reputation:** Instead of broadcasting exact scores, broadcast bucket: {low, medium, high, trusted}. This limits precision available to adversaries.

2. **Differential Privacy on Reputation:** Add calibrated noise to broadcast scores:
   ```
   R_broadcast = R_true + Laplace(0, epsilon)
   ```
   With epsilon = 0.1, the noise is small enough not to affect aggregation quality but prevents precise targeting.

3. **Homomorphic Reputation:** Use additively homomorphic encryption for reputation updates. Each node's true score is encrypted; only aggregated results are decrypted. This is computationally expensive and deferred to future work.

### Recommendation

For the current ESP32-C6 deployment, use **bucketed reputation** (option 1) as it has zero computational overhead. The four buckets map naturally to the existing threshold structure:

| Bucket | Score Range | Meaning |
|--------|------------|---------|
| Banned | [0.0, 0.2) | Excluded from consensus |
| Low | [0.2, 0.4) | Recent join or recovering from penalty |
| Medium | [0.4, 0.7) | Established but not yet trusted |
| Trusted | [0.7, 1.0] | Full participation including regime votes |

---

## v20 Update: Reputation^3 Scaling and Slander Amplification

### Context: Multimodal Reputation Weighting (Phase 2)

The v20 Cognitive Mesh introduces **reputation^3 weighting** in multimodal temporal attention fusion. This creates a steep influence curve:
- High-reputation nodes (R=1.0): Influence = 1.0^3 = 1.0
- Low-reputation nodes (R=0.1): Influence = 0.1^3 = 0.001 (1000x less)

**Benefit:** Strengthens Byzantine resistance (INV-2 Sybil, INV-3 Collusion) by drastically reducing adversarial influence.

### Slander Amplification Risk

With steeper weighting, slander attacks have **amplified impact** on high-reputation nodes:

**Scenario:** A trusted bridge node (R=0.9) gets slandered, dropping to R=0.7
- **Before (linear weighting):** Influence drops 0.9 → 0.7 = 22% reduction
- **After (rep^3 weighting):** Influence drops 0.9^3 → 0.7^3 = 0.729 → 0.343 = **53% reduction**

This creates an incentive for adversaries to coordinate slander campaigns against high-reputation nodes (bridges in zoned topologies).

### Mitigation: Median PeerEval with Reputation^3 Cap

**Enhanced Defense (v20):**

1. **Median PeerEval aggregation** (as designed above) prevents < n/3 slanderers from controlling the median
2. **Per-node influence cap:** Limit max influence to `rep^3 * global_constant` where constant bounds single-node contribution
3. **Bucketed reputation for gossip:** Broadcast buckets (Low/Medium/Trusted) instead of exact scores to prevent precise targeting

**Implementation in multimodal.rs:**
```rust
// Reputation weighting with cap
let rep_cubed = (reputation * reputation * reputation).min(MAX_INFLUENCE);
let final_weight = time_weight * rep_cubed;
```

Where `MAX_INFLUENCE = 0.85` ensures no single node dominates (even at R=1.0).

### Empirical Validation (Sensitivity Analysis)

Testing across swarm sizes [10, 25, 50, 100] with exponents [1.5, 2.0, 2.5, 3.0, 3.5, 4.0]:

| Swarm Size | Exponent | Gini Coefficient | Slander Risk |
|------------|----------|------------------|--------------|
| 10         | 3.0      | 0.315           | ✅ Safe (<0.7) |
| 25         | 3.0      | 0.336           | ✅ Safe       |
| 50         | 3.0      | 0.354           | ✅ Safe       |
| 100        | 3.0      | 0.364           | ✅ Safe       |

**Finding:** Gini coefficients stay well below the 0.7 "echo chamber" threshold, indicating influence is distributed enough to resist coordinated slander (no single group can dominate median PeerEval).

**Adaptive Exponent Recommendation:**
- Small swarms (<20 nodes): Use exponent=2.0 for greater diversity resistance
- Medium swarms (20-50): Use exponent=3.0 (current v20 default)
- Large swarms (>50): Use exponent=3.5 (cap at 3.5 to avoid over-amplification)

See: `docs/SENSITIVITY_ANALYSIS.md` for full empirical results.

### Updated Action on Slander Detection

With rep^3 weighting, slander detection becomes **critical for bridge nodes**:

1. **Immediate alert** if a Trusted node (R≥0.8) drops >0.2 in single round
2. **Lamarckian recovery:** Restore pre-slander reputation from NVRAM if slander pattern detected
3. **Adaptive exponent damping:** Temporarily lower exponent to 2.0 during suspected slander campaign
4. **Bridge redundancy:** Ensure multiple bridges per zone pair to prevent single-point-of-failure

**See:** `docs/COGNITIVE_MESH_ROADMAP.md` Phase 3 for bridge resilience mechanisms.

---

## Connection to Existing Defenses

| Component | How Anti-Slander Integrates |
|-----------|---------------------------|
| L2 Reputation | Median PeerEval replaces mean PeerEval |
| L4 Trimmed Mean | Unaffected (uses raw reputation weights, not PeerEval) |
| L5 ZK Audit | Unaffected (audit selection is deterministic, not reputation-dependent) |
| INV-4 Regime Gate | Uses "Trusted" bucket (R > 0.8) for vote eligibility |
| INV-1 Bounded Influence | Reputation weighting bounds any single node's influence |

## Implementation Status

| Feature | Status |
|---------|--------|
| Median PeerEval | Not yet implemented (current: implicit in simulation) |
| Reputation-weighted median | Design only |
| Slander detection heuristic | Design only |
| Bucketed reputation broadcast | Design only |
| Differential privacy on reputation | Future work |
| Homomorphic reputation | Future work |

## References

- PAC Reputation Bounds: `../../RaaS_Extras/docs/theory/PAC_REPUTATION_BOUNDS.md`
- Class C Defense: `docs/security/CLASS_C_DEFENSE.md`
- Security Invariants: `docs/security/INVARIANTS.md`
