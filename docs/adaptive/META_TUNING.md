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
    }
}
```

## Open Questions

1. **Should tuning be consensus-driven?** Currently each node tunes independently. Divergent parameters could cause split-brain behavior. A lightweight consensus on parameter ranges (e.g., all nodes broadcast their `gamma` and use the median) would prevent this.

2. **Adversarial tuning manipulation:** A Byzantine node could try to influence the tuner by artificially inflating `ban_rate` (via slander) or `drift_variance` (via large updates). The bounded clamping prevents catastrophic mistuning, but sophisticated attacks on the tuning signal are an open research question.

3. **Regime-dependent tuning:** Should `gamma` differ between Calm and Storm regimes? Storm's 30s round interval means reputation updates happen much faster; a lower `gamma` might be appropriate to avoid over-reaction.

## Status

- Design: Complete
- Implementation: Stub only (not integrated into runtime)
- Testing: Simulated in Gauntlet harness (static parameters used)
- Priority: Low (current static parameters work well for the target deployment)
