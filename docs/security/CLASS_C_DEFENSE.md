# Class C (APT/Colluding) Defense Strategy

## Adversary Profile

A Class C attacker is the most sophisticated adversary QRES considers:

| Capability | Description |
|-----------|-------------|
| **Collusion** | Controls multiple nodes that coordinate off-channel |
| **Reputation Farming** | Behaves honestly for $T$ rounds to build reputation, then attacks |
| **Adaptive Timing** | Attacks during regime transitions (PreStorm → Storm) |
| **Slander** | Gives false PeerEval scores to honest nodes to lower their reputation |
| **Budget** | Constrained to $< n/3$ of the swarm (safety assumption) |

## Attack Vectors and Defenses

### 1. Reputation Farming → Burst Attack

**Attack:** Colluding nodes behave honestly for $T_{farm}$ rounds to build R ≈ 1.0, then simultaneously inject poisoned updates.

**Defense layers:**

| Layer | Mechanism | Effect |
|-------|-----------|--------|
| L4 Trimmed Mean | Coordinate-wise trimming | Even with R=1.0, each node's update is trimmed if it's an outlier on any coordinate |
| L2 Reputation | Asymmetric penalties | One bad round costs -0.08 (drift) or -0.15 (ZK fail), while earning back requires +0.02 per round. Recovery from one attack takes 4-8 rounds |
| L5 ZK Audit | Stochastic audit | If the poisoned update diverges from the Q16.16 deterministic path, the audit catches it |

**Quantitative bound:** With $f$ colluding nodes at max reputation R=1.0 and $n-f$ honest nodes at R=0.8:
$$\text{Max influence} = \frac{f \cdot 1.0}{f \cdot 1.0 + (n-f) \cdot 0.8}$$

For $f = n/3$: influence = $\frac{n/3}{n/3 + 2n/3 \cdot 0.8}$ = $\frac{1/3}{1/3 + 8/15}$ = $\frac{5}{13}$ ≈ 0.385

This is bounded below 0.5 (majority), so colluders cannot dominate the weighted average even at maximum reputation.

### 2. Slander Attack (False PeerEval)

**Attack:** Colluding nodes assign PeerEval = 0 to targeted honest nodes to drive their reputation below $\tau_{ban}$.

**Defense:** PeerEval is computed locally by each node based on how much the evaluated node's update improves the local model. Slander requires the attacker to claim a specific node's update made things worse.

**Mitigation strategy:**
1. **Median PeerEval:** Instead of averaging PeerEval scores across evaluators, use the median. With $f < n/3$, the median is always controlled by honest evaluators.
2. **Reputation-weighted PeerEval:** Weight each evaluator's score by their own reputation. Low-rep nodes (likely Byzantine) have less influence on others' evaluations.
3. **Slander detection:** If node $i$ consistently rates node $j$ as 0 while the swarm median is > 0.5, flag node $i$ for potential slander. This is documented in `REPUTATION_PRIVACY.md`.

**PAC bound for slander resistance:** With median aggregation and $f < n/3$, the false PeerEval has zero influence on the honest node's reputation update. The honest node's reputation evolves as if only honest evaluators existed.

### 3. Regime Manipulation

**Attack:** Colluding nodes report false entropy spikes to force the swarm into Storm regime, draining battery.

**Defense (INV-4):** The `RegimeConsensusGate` requires:
- At least 3 nodes with R > 0.8 confirming entropy spike
- Entropy derivative must exceed the threshold for each voting node
- Votes expire after 10 rounds (prevents stale vote accumulation)

**Why this works:** Colluding nodes that just joined the swarm have R = 0.5 (below the 0.8 vote threshold). Farming R > 0.8 requires $(0.8 - 0.5) / 0.02 = 15$ consecutive valid ZKP submissions, giving the swarm time to establish trust.

### 4. Timing Attack on Regime Transitions

**Attack:** Attack during PreStorm → Storm transition when the swarm is reconfiguring TWT schedules and may have inconsistent state.

**Defense:**
- State machine transitions are atomic per-node (no partial states)
- The consensus gate requires quorum before Storm activation
- PreStorm is a safe intermediate state with 10-minute TWT (not the 30s Storm interval)
- If the consensus gate rejects Storm, the detector falls back to PreStorm

### 5. Energy Exhaustion via Unnecessary Storm

**Attack:** Even with regime consensus, legitimate entropy spikes could be manufactured by the attacker controlling the environment (e.g., jamming the RF channel to create real entropy).

**Defense:**
- Battery management enforces minimum brownout thresholds regardless of regime
- Solar harvest rate monitoring: if $P_{solar} < \bar{P}_{storm}$, the node auto-downgrades to Calm
- This is a hardware-layer defense that cannot be circumvented by software attacks

## Residual Risk Assessment

| Attack | Detection Time | Residual Impact | Mitigation |
|--------|---------------|-----------------|------------|
| Reputation farming → burst | 1 round (drift penalty) | Single-round drift ≤ 0.385× honest | Trimmed mean bounds single-round impact |
| Sustained mimicry ($\mu_B \geq 0.2$) | Never (by reputation) | Bounded by trimming | ZK audit every 50 rounds, trimming every round |
| Slander (with median PeerEval) | N/A (slander has no effect) | Zero | Median PeerEval is Byzantine-robust for $f < n/3$ |
| Regime manipulation | Prevented by consensus gate | Zero (Storm requires trusted quorum) | Gate requires R > 0.8 + entropy confirmation |
| Energy exhaustion | Hardware-layer defense | Limited to solar margin | Auto-downgrade when solar insufficient |

## Open Problems

1. **Adaptive $\gamma$:** Should the EMA learning rate increase when attacks are suspected? This could speed detection but also increases false positive risk. See `META_TUNING.md`.

2. **Cross-swarm attacks:** If an attacker builds reputation in one swarm and transfers to another, the reputation system must not transfer scores blindly. Each swarm maintains independent reputation.

3. **Sybil cost model:** The current analysis assumes Sybil nodes start at R=0.5. If Sybil creation cost is very low (cheap hardware), the attacker can create many nodes to approach the $n/3$ threshold. Physical attestation (e.g., solar panel capacity verification) could raise Sybil cost.

## Recommendations for Deployment

1. Set `min_trusted_confirmations = max(3, ceil(n_honest * 0.15))` to scale with swarm size
2. Monitor for reputation clustering: if many nodes have suspiciously similar reputation trajectories, flag for manual review
3. Log all ZK audit failures with full proof transcripts for post-hoc forensic analysis
4. Maintain per-swarm reputation (no cross-swarm reputation transfer)
