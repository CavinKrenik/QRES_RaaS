# QRES Threat Model

## 1. System Assets

| Asset | Description | Confidentiality | Integrity | Availability |
|-------|-------------|-----------------|-----------|--------------|
| Model weights | Consensus state `theta` | Medium | **Critical** | High |
| Reputation scores | Per-node trust `R_i` | Low | **Critical** | High |
| Battery state | Energy pool `B_i(t)` | Low | High | **Critical** |
| Gene bytecode | 74-byte evolved strategies | Low | **Critical** | Medium |
| Cryptographic keys | ed25519 signing keys | **Critical** | **Critical** | High |
| Entropy history | Regime detection state | Low | High | Medium |

## 2. Adversary Classes

### Class A: Oblivious (Static) Adversary

**Capabilities:**
- Controls up to `f < n/3` nodes
- Injects attacks independently of honest node states
- No access to honest updates before crafting attacks
- Each controlled node acts independently

**Attack Strategies:**
| Attack | Description | Detection Difficulty |
|--------|-------------|---------------------|
| Constant Bias | `theta_i* = theta_i + b` (fixed offset) | Low - consistent outlier |
| Sign Flip | `theta_i* = -theta_i` (negate updates) | Low - large deviation |
| Gaussian Noise | `theta_i* = theta_i + N(0, sigma)` | Medium - statistical |

**Mitigations:**
- L4 (Trimmed Mean): clips outliers per coordinate
- L2 (Reputation): decays score on detected drift
- Combined: Class A nodes banned within ~18 rounds

### Class B: Adaptive Adversary

**Capabilities:**
- All of Class A
- Can observe honest updates before crafting attacks (e.g., via eavesdropping)
- Can time attacks to exploit protocol timing
- Individual node adaptation (no coordination)

**Attack Strategies:**
| Attack | Description | Detection Difficulty |
|--------|-------------|---------------------|
| Label Flip | Target specific output dimensions | Medium |
| Mimicry (Sleeper) | Honest for T_0 rounds, then attack | High - delayed onset |
| Reputation Farming | Alternate honest/attack rounds | High - maintains R > rho_min |

**Mitigations:**
- L4 (Trimmed Mean): per-round defense catches attack rounds
- L2 (Reputation): temporal evidence accumulates across rounds
- L5 (ZK Audit): stochastic compliance checks catch determinism violations

### Class C: Colluding (APT) Adversary

**Capabilities:**
- All of Class B
- Byzantine nodes coordinate updates before submission
- Shared strategy: maximize drift while staying within trimming bounds
- Can execute multi-phase campaigns:
  1. **Reputation farming** (months of honest behavior)
  2. **Single catastrophic steering** (rare, high-impact attack)
  3. **Slander campaigns** (false negative PeerEval against honest nodes)
  4. **Timing-based regime forcing** (coordinated noise to trigger Storm)

**Attack Strategies:**
| Attack | Description | Detection Difficulty |
|--------|-------------|---------------------|
| Coordinated Bias | All Byzantine submit same poisoned update | High - appear as "inliers" |
| Quiet Collusion | Small bias below detection threshold, sustained | Very High |
| Slander | False negative PeerEval against honest nodes | High |
| Regime Manipulation | Coordinated entropy injection to force Storm | Medium |
| Long-Horizon Farming + Strike | Months honest, then single max-damage round | Very High |

**Mitigations:**
- Reputation-weighted influence (continuous, not binary)
- Regime consensus gate (Storm requires trusted quorum)
- ZK stochastic audit (compliance tax)
- Cross-temporal analysis (behavioral consistency tracking)
- **Residual risk**: Quiet collusion below detection threshold remains partially effective

## 3. Attack Surfaces

### 3.1 Consensus Layer
- **Surface**: Model update submission via gossip
- **Threat**: Byzantine updates poisoning consensus
- **Defense**: L2 + L4 fusion (reputation-weighted trimmed mean)

### 3.2 Reputation Layer
- **Surface**: PeerEval voting mechanism
- **Threat**: Slander (false negative evaluations)
- **Defense**: Verifiable reputation updates (L5 ZK proofs)

### 3.3 Energy Layer
- **Surface**: Regime state machine transitions
- **Threat**: Forced Storm transitions causing battery exhaustion
- **Defense**: Regime consensus gate (trusted quorum required)

### 3.4 Network Layer
- **Surface**: Gossip protocol message exchange
- **Threat**: Eclipse attacks, message suppression, replay
- **Defense**: ed25519 authentication (L1), monotonic epoch binding

### 3.5 Persistence Layer
- **Surface**: Flash memory serialization (Lamarckian persistence)
- **Threat**: Corrupted state on reboot
- **Defense**: BLAKE3 hash verification of serialized state

## 4. Assumptions

### Cooperative Deployment
- Node operators have economic incentive to participate honestly
- Physical access to nodes is controlled
- Network infrastructure (mesh/WiFi) is untrusted but available

### Hostile Deployment
- Any node may be compromised at any time
- Attacker budget is bounded (energy cost of participation)
- f < n/3 Byzantine bound is maintained (enrollment-time assumption)
- No hardware-level side-channel attacks (future work)

## 5. Residual Risk

| Risk | Severity | Likelihood | Status |
|------|----------|------------|--------|
| Quiet collusion below threshold | Medium | Low | Partially mitigated by reputation weighting |
| Long-horizon farming + strike | High | Very Low | Mitigated by ZK audit + per-round trimming |
| Hardware compromise (key extraction) | Critical | Very Low | Out of scope (hardware trust) |
| >n/3 Byzantine takeover | Critical | Low | Fundamental BFT limit, no mitigation |
| Slander campaigns | Medium | Medium | Mitigated by verifiable reputation (Phase 5) |
