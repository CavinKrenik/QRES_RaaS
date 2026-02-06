# TWT (Target Wake Time) Integration for QRES

**Status:** Implemented (Simulation Mode)
**Module:** `qres_core::power::twt_scheduler`
**Version:** v19.1.0

---

## ⚠️ Implementation Note

**Current Status:** TWT scheduling uses `MockRadio` abstraction for simulation-based verification. Energy measurements are **theoretical** based on ESP32-C6 datasheet specifications:
- **Active TX/RX:** ~230 mW
- **Idle Listen:** ~80 mW  
- **TWT Sleep:** ~35 mW

**Hardware Validation:** Physical integration with native Wi-Fi 6 TWT drivers (ESP32-C6, ESP32-S3) planned for Q2 2026. Current 82% sleep savings are simulation-verified; real-world performance may vary based on AP capabilities, channel conditions, and TWT service period negotiation.

---

## Overview

Target Wake Time (TWT) is a Wi-Fi 6 (802.11ax) mechanism that allows stations to negotiate scheduled sleep/wake cycles with the access point. This module integrates TWT scheduling with QRES's regime-aware silence protocol, enabling nodes to coordinate radio sleep with swarm learning state.

Currently operates in **simulation mode** using `MockRadio` for testing without Wi-Fi 6 hardware.

---

## Cross-Layer Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Application Layer (QRES)                               │
│  ┌─────────────────┐    ┌──────────────────────────┐    │
│  │ RegimeDetector  │───►│ TWTScheduler             │    │
│  │ Calm/PreStorm/  │    │ - Interval selection     │    │
│  │ Storm           │    │ - Sleep/wake decisions   │    │
│  └─────────────────┘    │ - Gossip batching        │    │
│                         └──────────┬───────────────┘    │
├────────────────────────────────────┼────────────────────┤
│  MAC Layer (802.11ax TWT)          │                    │
│  ┌────────────────────────────────┐│                    │
│  │ TWT Agreement Negotiation      ││                    │
│  │ - Service Period (SP)          ││                    │
│  │ - Wake Interval                ◄┘                    │
│  │ - TWT Flow Identifier           │                    │
│  └─────────────────────────────────┘                    │
├─────────────────────────────────────────────────────────┤
│  Physical Layer                                         │
│  ┌─────────────────────────────────┐                    │
│  │ Radio Power States:             │                    │
│  │ - Active TX: ~220 mW            │                    │
│  │ - Idle/Listen: ~80 mW           │                    │
│  │ - TWT Sleep: ~5 mW              │                    │
│  └─────────────────────────────────┘                    │
└─────────────────────────────────────────────────────────┘
```

---

## Node Roles

| Role        | Behavior                                        | Use Case                          |
|-------------|------------------------------------------------|-----------------------------------|
| **Sentinel**  | Always awake. Monitors entropy, broadcasts emergency wakes. | Gateway nodes, powered devices    |
| **OnDemand**  | Sleeps indefinitely. Wakes on Sentinel broadcast or Storm. | Battery-constrained, rare updates |
| **Scheduled** | Periodic TWT wake/sleep governed by regime intervals.     | Standard swarm participants       |

---

## Regime-Aware Sleep Intervals

The TWTScheduler automatically adjusts wake intervals based on the current regime:

| Regime      | Wake Interval | Rationale                                      |
|-------------|---------------|------------------------------------------------|
| **Calm**    | 4 hours       | Low entropy. Minimal coordination needed. Deep conservation. |
| **PreStorm**| 10 minutes    | Entropy derivative rising. Prepare for convergence round.  |
| **Storm**   | 30 seconds    | Active federated learning. Frequent sync required.         |

Transitions to a more urgent regime (Calm→PreStorm, PreStorm→Storm) trigger **immediate wake** regardless of the scheduled interval.

---

## Reputation-Weighted Sleep Intervals

Sleep intervals scale with the node's reputation score (0.0–1.0), rewarding reliable nodes with longer sleep and forcing low-reputation nodes to stay awake more.

### Formula

```
floor_fraction = 1.0 / 5.0 = 0.2
weight = floor_fraction + reputation * (1.0 - floor_fraction)
effective_interval = base_interval * weight
```

### Examples (Calm regime, base = 4h)

| Reputation | Weight | Effective Interval | Behavior                          |
|------------|--------|-------------------|-----------------------------------|
| 1.0        | 1.0    | 4h                | Full trust, maximum sleep         |
| 0.9        | 0.92   | 3h 41m            | Proven reliable                   |
| 0.5        | 0.6    | 2h 24m            | Mid-tier, moderate contribution   |
| 0.2        | 0.36   | 1h 26m            | Low trust, must prove reliability |
| 0.0        | 0.2    | 48m               | Untrusted, floor (base/5)         |

### Properties

- **Monotonic**: Higher reputation always yields longer sleep
- **Floor guarantee**: Even reputation=0.0 gets base/5, never zero (prevents starvation)
- **Natural staggering**: Different reputations → different wake times → no blind spots
- **Regime-aware**: The weighting applies to whichever base interval the current regime selects
- **Dynamic**: `set_reputation()` recalculates the interval immediately

### API

```rust
// Create with specific reputation
let node = TWTScheduler::with_reputation(NodeRole::Scheduled(cfg), 0.7);

// Update reputation dynamically (e.g., after successful ZKP verification)
node.set_reputation(0.85, now_ms);

// Pure function for interval calculation
let interval = calculate_weighted_interval(base_ms, reputation);
```

---

## Power Savings Calculations

### Energy Model

| State        | Power (mW)     | Components                    |
|-------------|----------------|-------------------------------|
| Active TX   | 220            | Radio transmit                |
| Idle/Listen | 80 + 150 = 230 | Radio idle + CPU active       |
| TWT Sleep   | 5 + 30 = 35    | Radio sleep + CPU idle        |

### Savings Formula

```
savings_percent = (baseline_energy - actual_energy) / baseline_energy * 100

baseline_energy = (radio_idle + cpu_active) * total_hours    [always-on]
actual_energy   = (radio_idle + cpu_active) * awake_hours
                + (radio_sleep + cpu_idle)  * sleep_hours
                + radio_active_tx * tx_events * tx_duration
```

### Expected Savings by Regime

| Scenario                      | Radio-Off Ratio | Energy Savings |
|-------------------------------|----------------|----------------|
| 24h Calm only                 | >95%           | ~82%           |
| 18h Calm + 2h Storm + 4h Calm| >75%           | ~65%           |
| Continuous Storm              | ~0%            | ~0%            |

---

## Gossip Batching

When the radio is asleep, outgoing `GhostUpdate` packets are queued in the `GossipBatchQueue`:

1. **Enqueue**: Messages accumulate during sleep (bounded by `max_batch_size`)
2. **Overflow Policy**: Oldest messages dropped when queue is full
3. **Burst TX**: On wake, all queued messages are transmitted in a burst
4. **TX Accounting**: Burst energy cost is tracked per-message (~2ms TX time each)

This batching aligns with QRES's existing philosophy of bandwidth-efficient communication.

---

## Trade-offs: Latency vs Battery Life

| Setting              | Latency Impact          | Battery Impact           |
|---------------------|------------------------|--------------------------|
| Sentinel role       | Lowest (always awake)  | Highest (no savings)     |
| Storm interval (30s)| Low (30s max delay)    | Moderate savings         |
| PreStorm (10min)    | Moderate               | Good savings             |
| Calm (4h)           | High (hours of delay)  | Excellent savings        |
| OnDemand role       | Variable (emergency OK)| Best savings             |

The regime-aware system resolves this trade-off dynamically: during Calm (low entropy), latency is acceptable because there's little to communicate. During Storm (high entropy), latency is minimized because updates are valuable.

---

## Sentinel Emergency Wake Protocol

When a Sentinel detects a Storm regime:

1. Sentinel calls `emergency_wake()` on all OnDemand peers
2. OnDemand nodes wake immediately (simulated: 0ms, hardware target: <1s)
3. Queued gossip messages are burst-transmitted
4. All nodes operate in Storm intervals (30s) until Calm returns

In simulation, emergency response is instant. On real hardware, the response time is bounded by the OnDemand node's TWT listen interval (configurable).

---

## Testing

### Unit Tests (17 tests in `twt_scheduler.rs`)

- Interval calculation for all regimes
- Sentinel always-on behavior
- Scheduled sleep/wake cycles
- OnDemand emergency wake
- Storm force-wake from sleep
- Gossip queue batching and overflow
- Mock radio energy tracking
- Power savings assertions
- Deterministic jitter verification

### Integration Tests (3 tests in `twt_integration_test.rs`)

- **24-hour swarm simulation**: 10 nodes (1 Sentinel + 2 OnDemand + 7 Scheduled), verifies >40% sleep ratio for Scheduled nodes during mixed regime operation
- **Emergency wake response**: Validates <1s (instant in simulation) emergency wake
- **Regime cycle correctness**: Full Calm→PreStorm→Storm→Calm interval verification

### Running Tests

```bash
# Unit tests only
cargo test -p qres_core --features std -- power::

# Integration tests only
cargo test -p qres_core --features std --test twt_integration_test

# All tests
cargo test -p qres_core --features std
```

---

## Future Work: Hardware Integration

### Phase 1: Linux Simulation (Current)
- `MockRadio` simulates radio on/off without hardware
- Deterministic timestamps for reproducible testing
- Energy estimates based on published Wi-Fi 6 power data

### Phase 2: ESP32-C6 / Pi Zero W 2
1. Replace `MockRadio` with platform-specific radio control trait
2. Use ESP32's `esp_wifi_sta_twt_setup()` for real TWT negotiation
3. Calibrate `EnergyProfile` against USB power meter readings
4. Correlate `total_energy_consumed` API with physical mWh

### Phase 3: Multi-AP TWT Coordination
- Stagger TWT wake times across a mesh to reduce contention
- Use QRES regime consensus to coordinate AP-level TWT parameters
- Explore broadcast TWT for efficient Sentinel→OnDemand wake signaling

---

## API Reference

```rust
// Create schedulers
let sentinel = TWTScheduler::new_sentinel();
let on_demand = TWTScheduler::new_on_demand();
let scheduled = TWTScheduler::new_scheduled(); // Default Calm interval

// Custom configuration
let cfg = TWTConfig {
    base_interval_ms: 4 * 60 * 60 * 1000, // 4 hours
    jitter_enabled: true,                   // ±10% wake time variation
    max_batch_size: 64,                     // Max queued messages
};
let custom = TWTScheduler::new(NodeRole::Scheduled(cfg));

// Regime integration (call when RegimeDetector changes)
scheduler.update_regime(Regime::Storm, now_ms);

// Tick the scheduler (call periodically)
let pending = scheduler.tick(now_ms);
if pending > 0 {
    let batch = scheduler.drain_batch(); // Burst-send queued messages
}

// Check transmit readiness
if scheduler.should_transmit(now_ms) {
    send_gossip(update);
} else {
    scheduler.enqueue_gossip(update); // Queue for later
}

// Get power metrics
let metrics = scheduler.get_metrics(now_ms);
println!("Sleep ratio: {:.1}%", metrics.radio_sleep_ratio * 100.0);
println!("Energy savings: {:.1}%", metrics.savings_percent);
```
