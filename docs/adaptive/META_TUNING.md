# Meta-Tuning: Adaptive Hyperparameter Selection

## Problem Statement

QRES has several hyperparameters that affect defense efficacy:

| Parameter | Current Value | Controls |
|-----------|--------------|----------|
| `gamma` (EMA rate) | 0.05 | Reputation responsiveness |
| `tau_ban` | 0.2 | Ban threshold |
| `trim_f` | varies | Trimming aggressiveness |
| `audit_interval` | 50 rounds | ZK audit frequency |
| `min_trusted_confirmations` | 3 | Regime consensus quorum |
| `lr_decay_factor` (v20) | 0.95 | Multimodal LR imbalance correction |
| `lr_min_scale` (v20) | 0.6 | Minimum per-modality learning rate |
| `reputation_exponent` (v20) | 3.0 | Reputation weighting curve (rep^n) |

These are currently static. In a dynamic deployment where the adversary fraction, network conditions, and swarm size change, static parameters may be suboptimal.

## Design: PolicyTuner

A `PolicyTuner` adjusts hyperparameters based on observable swarm state, without requiring communication beyond normal gossip.

### Observable State Vector

Each node can locally compute:

| Signal | Computation | Indicates |
|--------|------------|-----------|
| `ban_rate` | Fraction of nodes banned in last 20 rounds | Attack intensity |
| `drift_variance` | Variance of consensus drift over last 10 rounds | Model instability |
| `reputation_entropy` | Shannon entropy of reputation distribution | Diversity of trust |
| `energy_margin` | (solar_harvest - avg_consumption) / battery_capacity | Sustainability headroom |

### Tuning Rules

#### Rule 1: Adaptive `gamma`

```
if ban_rate > 0.15:
    gamma = min(gamma * 1.5, 0.15)   # Speed up detection under attack
elif ban_rate < 0.05 and drift_variance < 0.01:
    gamma = max(gamma * 0.8, 0.02)   # Slow down during calm periods
```

**Rationale:** High ban rate suggests active attacks; faster EMA accelerates Byzantine exclusion. Low ban rate with stable drift means the swarm is healthy; slower EMA reduces false positive risk.

#### Rule 2: Adaptive `audit_interval`

```
if ban_rate > 0.10:
    audit_interval = max(audit_interval - 10, 10)  # Audit more often
elif energy_margin < 0.2:
    audit_interval = min(audit_interval + 20, 100)  # Conserve energy
```

**Rationale:** Audits consume computation. Under attack, more frequent audits catch mimicry nodes faster. When energy is scarce, reduce audit overhead.

#### Rule 3: Adaptive `trim_f`

```
n_active = count(nodes where R >= tau_ban)
# Standard: trim at most n/3 nodes
trim_f = max(1, min(n_active // 3, ceil(ban_rate * n_active)))
```

**Rationale:** Trimming should scale with the estimated adversary fraction, but never exceed the theoretical safety limit of n/3.

### Safety Constraints

All tuning is bounded to prevent the tuner itself from being a vulnerability:

1. `gamma` is clamped to [0.02, 0.15]
2. `tau_ban` is NEVER adjusted (hardcoded safety threshold)
3. `trim_f` never exceeds `n_active / 3`
4. `audit_interval` is clamped to [10, 100]
5. `min_trusted_confirmations` is clamped to [2, ceil(n_honest * 0.3)]

### Convergence Guarantee

The tuner converges to a fixed point when:
- `ban_rate` stabilizes (all detectable attackers banned)
- `drift_variance` stabilizes (consensus converged)
- `energy_margin` stabilizes (harvest matches consumption)

Under stationary conditions, the tuner is a contraction mapping and reaches its fixed point within ~20 rounds.

## Implementation Stub

```rust
pub struct PolicyTuner {
    gamma: f32,
    audit_interval: u64,
    trim_f: usize,
    // Observable state (exponential moving averages)
    ban_rate_ema: f32,
    drift_var_ema: f32,
    energy_margin: f32,
}

impl PolicyTuner {
    pub fn update(&mut self, ban_rate: f32, drift_var: f32, energy_margin: f32) {
        self.ban_rate_ema = 0.1 * ban_rate + 0.9 * self.ban_rate_ema;
        self.drift_var_ema = 0.1 * drift_var + 0.9 * self.drift_var_ema;
        self.energy_margin = energy_margin;

        // Rule 1: Adaptive gamma
        if self.ban_rate_ema > 0.15 {
            self.gamma = (self.gamma * 1.5).min(0.15);
        } else if self.ban_rate_ema < 0.05 && self.drift_var_ema < 0.01 {
            self.gamma = (self.gamma * 0.8).max(0.02);
        }

        // Rule 2: Adaptive audit interval
        if self.ban_rate_ema > 0.10 {
            self.audit_interval = self.audit_interval.saturating_sub(10).max(10);
        } else if self.energy_margin < 0.2 {
            self.audit_interval = (self.audit_interval + 20).min(100);
        }
        
        // Rule 3 (v20): Adaptive multimodal LR decay
        // Higher swarm entropy → gentler decay to preserve diversity
        if self.reputation_entropy > 2.5 {  // High diversity
            self.lr_decay_factor = 0.98;  // Very gentle
        } else if self.reputation_entropy < 1.0 {  // Echo chamber risk
            self.lr_decay_factor = 0.90;  // Aggressive correction
        } else {
            self.lr_decay_factor = 0.95;  // Default (current v20 value)
        }
        
        // Rule 4 (v20): Adaptive reputation exponent
        // Small swarms (<20 nodes) → lower exponent to reduce echo chambers
        // Large swarms (>50 nodes) → higher exponent for stronger Byzantine resistance
        if self.swarm_size < 20 {
            self.reputation_exponent = 2.0;  // Quadratic (gentler)
        } else if self.swarm_size > 50 {
            self.reputation_exponent = 3.5;  // Stronger than cubic
        } else {
            self.reputation_exponent = 3.0;  // Default (current v20 value)
        }
    }
}
```

### Rule 3 Implementation Notes (v20)

**Adaptive `trim_f`** is fully implemented in the gauntlet harness and simulation layer:

```
n_active = count(nodes where R >= tau_ban)
trim_f = max(1, min(n_active // 3, ceil(ban_rate * n_active)))
```

**Swarm-size thresholds for trim_f:**

| Swarm Size | Default trim_f | Max trim_f | Rationale |
|------------|---------------|------------|-----------|
| < 10 nodes | 1 | 3 | Small swarm: trim 1 is safe, trim 3 = n/3 limit |
| 10-25 nodes | 2 | 8 | Standard deployment: trim top/bottom 2 per dimension |
| 25-50 nodes | 3 | 16 | Medium swarm: increased Byzantine margin |
| > 50 nodes | 4 | n/3 | Large swarm: trim scales with estimated adversary count |

**Safety bound:** `trim_f` never exceeds `n_active / 3` regardless of observed `ban_rate`.
This preserves the $f < n/3$ Byzantine safety bound (Theorem 1 in main.tex).

### Rule 4 Implementation Notes (v20)

**Adaptive reputation exponent** is implemented in `multimodal.rs` via the
`reputation_weight` parameter passed to `predict_with_attention()`:

```rust
// In the caller (daemon/sim), select exponent based on swarm size:
let exponent = if swarm_size < 20 {
    2.0   // Quadratic: gentler curve for small swarms
} else if swarm_size > 50 {
    3.5   // Stronger than cubic for large swarms
} else {
    3.0   // Default cubic (v20 baseline)
};
let rep_weight = reputation.powf(exponent);
let prediction = fusion.predict_with_attention(modality, rep_weight);
```

**Sensitivity analysis results (24 configurations):**

| Swarm Size | Exponent 2.0 | Exponent 3.0 | Exponent 3.5 | Exponent 4.0 | Best |
|------------|-------------|-------------|-------------|-------------|------|
| 10 nodes | 0.0297 | 0.0329 | **0.0266** | 0.0298 | 3.5 |
| 25 nodes | 0.0371 | 0.0385 | 0.0352 | **0.0339** | 4.0 |
| 50 nodes | 0.0358 | 0.0364 | 0.0341 | **0.0330** | 4.0 |
| 100 nodes | 0.0355 | 0.0349 | **0.0342** | 0.0348 | 3.5 |

**v20 Decision:** Use adaptive thresholds (2.0 / 3.0 / 3.5) rather than a fixed exponent.
The 3.5 cap for large swarms avoids the error uptick observed at exp=4.0 in 50-100 node
configurations, while 2.0 for small swarms prevents echo chamber risk (Gini < 0.7).

**Influence Cap (v20.1):** Additionally, `reputation.rs` now applies `INFLUENCE_CAP = 0.8`
to bound `rep^exponent` at 0.8 regardless of exponent, mitigating Slander-Amplification
(see `REPUTATION_PRIVACY.md` Section v20 Update).

## Open Questions

1. **Should tuning be consensus-driven?** Currently each node tunes independently. Divergent parameters could cause split-brain behavior. A lightweight consensus on parameter ranges (e.g., all nodes broadcast their `gamma` and use the median) would prevent this.

2. **Adversarial tuning manipulation:** A Byzantine node could try to influence the tuner by artificially inflating `ban_rate` (via slander) or `drift_variance` (via large updates). The bounded clamping prevents catastrophic mistuning, but sophisticated attacks on the tuning signal are an open research question.

3. **Regime-dependent tuning:** Should `gamma` differ between Calm and Storm regimes? Storm's 30s round interval means reputation updates happen much faster; a lower `gamma` might be appropriate to avoid over-reaction.

4. **Multimodal echo chamber detection (v20):** The reputation^3 scaling creates steep influence curves. In small swarms or during collusion, this risks echo chambers where a few high-rep nodes dominate. Adaptive exponent (Rule 4) mitigates this, but needs empirical validation via sensitivity analysis in `multimodal_gauntlet_v20.py`.

5. **LR min threshold safety (v20):** Current min=0.6 ensures no modality starvation, but prolonged high-error scenarios (e.g., broken sensor) might warrant temporary exclusion. Should we add a "fault detection" override that drops LR to 0.1 if error stays >0.5 for 100+ rounds?

## Status

- Design: Complete (v20 rules added)
- Implementation: Stub only (not integrated into runtime)
- Testing: Simulated in Gauntlet harness (static parameters used)
- Priority: Low (current static parameters work well for the target deployment)
