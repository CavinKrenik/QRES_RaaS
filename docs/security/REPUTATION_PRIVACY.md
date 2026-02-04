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

- PAC Reputation Bounds: `docs/theory/PAC_REPUTATION_BOUNDS.md`
- Class C Defense: `docs/security/CLASS_C_DEFENSE.md`
- Security Invariants: `docs/security/INVARIANTS.md`
