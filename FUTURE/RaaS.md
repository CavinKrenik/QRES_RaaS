# RaaS: Resource-Aware Agentic Swarm

**Status:** ✅ Implemented 
**Reference:** `../../RaaS_Extras/docs/theory/SNN_ENERGY_ANALYSIS.md` (21.9x energy reduction)

---

# Technical Synthesis

> **Implemented:** A **Biological-Inspired Autonomic System** mapping SNN energy advantages into swarm decision-making logic.

## 1. The Energy-Regime Feedback Loop (Phase 1 & 2)

The "Energy Gate" introduces a **physical constraint to a mathematical state**.

```rust
// In RegimeDetector::update()
if energy_pool.is_critical() && desired_regime == Regime::Storm {
    // FORCED DOWNGRADE: Node "wants" Storm but can't afford it
    return Regime::Calm;
}
```

**Result:** Prevents hardware brownouts during critical consensus rounds.

## 2. Strategic Silence & Vouching (Phase 3 & 4)

Leveraging Calm regime stability to save bandwidth.

| Metric | Value |
|--------|-------|
| **Bandwidth Savings** | ~80% during Calm (Verified) |
| **Logic** | `SilenceController`: Gates broadcast based on `utility < cost` |
| **States** | `Active` (Storm), `Alert` (Transition), `DeepSilence` (Calm/Stable) |

## 3. Social Dynamics & Free-Rider Mitigation (Phase 5)

Prevents nodes from selfishly sleeping while others do the work.

- **Storm Sleeper Penalty:** `-0.05` reputation/tick for sleeping during Storms.
- **Cure Gene Incentive:** `+0.03` reputation for responding to help requests.
- **Result:** Self-balancing system where altruism is mathematically optimal.

---

# Architecture & Implementation

## 1. Energy Accounting (`qres_core/src/resource_management.rs`)

Deterministic tracking of micro-joule equivalent units.

```rust
pub struct EnergyPool {
    current: u32,
    lifetime_consumption: u64, // For hardware calibration
}
```
**Phase 6 Telemetry:**  
`get_status` API now exposes `total_energy_consumed` for calibration against physical power meters (Pi Zero / ESP32).

## 2. Energy Profiles (`qres_core/src/resource_management.rs`)

Hot-swappable hardware profiles for accurate cost estimation.

| Profile | Predict Cost | Gossip TX | Idle Drain |
|---------|--------------|-----------|------------|
| `PI_ZERO_PROFILE` | 2 | 120 | 10 |
| `ESP32_PROFILE` | 1 | 200 | 5 |

## 3. Utility Logic (`qres_daemon/src/swarm_p2p.rs`)

The decision engine limits communication to high-value updates.

```rust
// Integrated into Publish Loop
let energy_ratio = energy_pool.ratio();
if !silence_controller.should_broadcast(entropy, reputation, energy_ratio, cost) {
    info!("Strategic Silence: Suppressing broadcast (low utility)");
    return; // Save Energy
}
// Strict Hardware Gate
if !energy_pool.spend(cost) {
    return; // Brownout prevention
}
```

---

# Verified Results (v19.0.0)

| Experiment | Status | Result | Findings |
|------------|--------|--------|----------|
| **SNN vs ANN Collapse** | ✅ Pass | SNN Survived | ANN swarms collapsed (<10% energy) while SNN retained >80% capacity. |
| **Strategic Silence** | ✅ Pass | Visualized | Center nodes successfully transition to `DeepSilence` (Grey) in `swarm_sim` verification. |
| **Free-Rider Mitigation** | ✅ Pass | Logic Verified | Reputation decay effectively punishes non-responsive nodes in Storms. |
| **Hardware Telemetry** | ✅ Ready | API Exposed | `/status` endpoint ready for USB power meter calibration. |

---

# Next Steps: Physical Calibration

Now that the logic is implemented and verified in simulation, the next phase is hardware-in-the-loop tuning.

1. **Deploy to Pi Zero 2 W**: Run `qres_daemon`.
2. **Attach Power Meter**: Measure accumulated mWh over 1 hour.
3. **Correlate**: Compare mWh vs `total_energy_consumed` from API.
4. **Tune**: Adjust `EnergyProfile` constants to match physical reality.
