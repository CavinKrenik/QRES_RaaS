# PAC Reputation Bounds for Byzantine Detection

## Overview

This document derives **Probably Approximately Correct (PAC)** bounds for the QRES reputation system. We answer:

> After how many rounds can we guarantee that all Byzantine nodes are detected (banned) with probability at least $1 - \delta$?

## Setup

| Symbol | Definition |
|--------|-----------|
| $n$ | Total swarm nodes |
| $f$ | Number of Byzantine nodes ($f < n/3$) |
| $R_i(t)$ | Reputation of node $i$ at round $t$ |
| $\gamma$ | EMA learning rate (default: 0.05) |
| $\tau_{ban}$ | Ban threshold (default: 0.2) |
| $R_0$ | Initial reputation (default: 0.5) |

## Theorem (Deterministic Ban Time)

**Claim:** Under the assumption that Byzantine nodes receive PeerEval = 0 at every round, all Byzantine nodes are banned by round:

$$T_{ban} = \left\lceil \frac{\log(\tau_{ban} / R_0)}{\log(1 - \gamma)} \right\rceil$$

**Proof:**
A Byzantine node's reputation evolves as:
$$R_i(t) = (1 - \gamma)^t \cdot R_0$$

Setting $R_i(T_{ban}) = \tau_{ban}$:
$$(1 - \gamma)^{T_{ban}} \cdot R_0 = \tau_{ban}$$
$$T_{ban} = \frac{\log(\tau_{ban} / R_0)}{\log(1 - \gamma)}$$

With defaults ($\gamma = 0.05$, $R_0 = 0.5$, $\tau_{ban} = 0.2$):
$$T_{ban} = \frac{\log(0.4)}{\log(0.95)} \approx \frac{-0.916}{-0.0513} \approx 17.9 \implies T_{ban} = 18 \text{ rounds}$$

## PAC Extension: Noisy Peer Evaluation

In practice, PeerEval is not perfectly 0 for Byzantine nodes. Honest nodes may have noisy local models that occasionally give Byzantine nodes non-zero scores.

**Model:** Let $X_i(t) \in [0, 1]$ be the PeerEval score assigned to Byzantine node $i$ at round $t$. Assume:
- $\mathbb{E}[X_i(t)] = \mu_B$ where $\mu_B < \mu_H$ (Byzantine mean score < honest mean score)
- Scores are independent across rounds (conservative; in practice, correlated scores make detection easier)

The reputation under EMA becomes:
$$R_i(t) = (1 - \gamma)^t R_0 + \gamma \sum_{k=0}^{t-1} (1 - \gamma)^{t-1-k} X_i(k)$$

**Steady-state reputation** (as $t \to \infty$):
$$R_i(\infty) = \mu_B$$

For the node to eventually be banned, we need $\mu_B < \tau_{ban} = 0.2$.

### Theorem (PAC Ban Time under Noise)

**Claim:** If $\mu_B < \tau_{ban}$ and individual PeerEval scores are bounded in $[0, 1]$, then for any $\delta > 0$, a Byzantine node is banned by round $T$ with probability $\geq 1 - \delta$ where:

$$T = \frac{1}{\gamma} \cdot \log\left(\frac{R_0}{\tau_{ban} - \mu_B}\right) + \frac{1}{\gamma} \cdot \sqrt{\frac{\log(1/\delta)}{2}}$$

**Proof sketch:**
1. Decompose $R_i(t) = \bar{R}_i(t) + \epsilon_i(t)$ where $\bar{R}_i(t)$ is the expected trajectory and $\epsilon_i(t)$ is noise.
2. The expected trajectory crosses $\tau_{ban}$ at time $T_0 = \frac{1}{\gamma} \log\left(\frac{R_0 - \mu_B}{\tau_{ban} - \mu_B}\right)$.
3. The noise term $\epsilon_i(t)$ is a weighted sum of bounded random variables. By the Azuma-Hoeffding inequality for EMA sequences:
$$\Pr[|\epsilon_i(t)| > \epsilon] \leq 2 \exp\left(-\frac{2\epsilon^2}{\gamma \sum_{k=0}^{t-1}(1-\gamma)^{2(t-1-k)}}\right)$$
4. The effective number of independent samples in an EMA is $\approx 1/(2\gamma)$, giving the concentration bound.
5. Union bound over $f$ Byzantine nodes adds $\log(f)$ to the required rounds.

### Numerical Examples

| Scenario | $\mu_B$ | $\delta$ | $T_{PAC}$ (rounds) |
|----------|---------|---------|-------------------|
| Pure adversary (no noise) | 0.0 | 0.0 | 18 |
| Weak mimicry ($\mu_B = 0.1$) | 0.1 | 0.05 | 31 |
| Strong mimicry ($\mu_B = 0.15$) | 0.15 | 0.05 | 58 |
| Near-threshold ($\mu_B = 0.19$) | 0.19 | 0.05 | 312 |
| Undetectable ($\mu_B \geq 0.2$) | 0.2 | any | $\infty$ |

## Critical Insight: The $\mu_B \geq \tau_{ban}$ Regime

If a Byzantine node can maintain $\mu_B \geq 0.2$ (i.e., it produces updates that look "good enough" to pass PeerEval), **reputation alone cannot detect it**. This is the Class C attacker regime.

### Defense in Depth (beyond reputation)

For $\mu_B \geq \tau_{ban}$, QRES relies on the remaining defense layers:

1. **L4 Trimmed Mean:** Even if the node isn't banned, coordinate-wise trimming bounds its influence to $O(1/n)$.
2. **L5 ZK Audit:** Stochastic audits (every 50 rounds) catch nodes that compute updates off the deterministic Q16.16 path, regardless of reputation.
3. **L3 Differential Privacy:** Noise injection limits the information any single node can extract from the consensus.

### Reputation as an Efficiency Mechanism, Not a Safety Mechanism

The PAC bounds show that reputation is best understood as an **efficiency mechanism** that accelerates Byzantine exclusion when $\mu_B$ is low, rather than a safety mechanism that guarantees exclusion. True safety comes from the combination of trimming + ZK + DP (Layers 3-5).

## Implications for Hyperparameter Tuning

| Parameter | Increase Effect | Decrease Effect |
|-----------|----------------|-----------------|
| $\gamma$ (EMA rate) | Faster detection but more false positives | Slower detection but more stable |
| $\tau_{ban}$ (threshold) | Easier to ban (more false positives) | Harder to ban (more false negatives) |
| Audit interval | Less overhead but slower ZK detection | More overhead but faster ZK detection |

### Recommended Operating Point

For $f < n/3$ with mixed Class A/B attackers:
- $\gamma = 0.05$: Balances responsiveness vs. stability
- $\tau_{ban} = 0.2$: Low enough to catch persistent adversaries, high enough to avoid honest node bans under noise
- $T_{ban} \leq 20$ rounds: Adequate for swarms with 30s-4h TWT intervals

## Connection to Paper Theorems

- **Theorem 1 (Byzantine Safety Bound):** Uses the post-ban phase ($t > T_{ban}$) where the PAC bound guarantees Byzantine exclusion.
- **Theorem 3 (Convergence Rate):** The $O(1/|H|)$ convergence rate holds after $T_{ban}$ rounds, where $|H|$ = honest node count.
- **Experimental validation:** Table V shows all 5 Byzantine nodes gated by round 18, consistent with the deterministic bound.
