# Virtual IoT Network Demo

**100-Node Sensor Mesh with Byzantine Fault Injection**

This example demonstrates QRES in a realistic edge computing scenario with:
- 100 simulated IoT sensor nodes
- Real-time consensus visualization
- Noise injection and Byzantine node simulation
- REST API for external monitoring
- Web dashboard (http://localhost:8080)

## Features

- **Deterministic Convergence:** Fixed-point consensus across heterogeneous nodes
- **Byzantine Tolerance:** Coordinate-wise trimmed mean aggregation
- **Regime Adaptation:** Automatic Calm/PreStorm/Storm transitions
- **Energy Awareness:** TWT-based sleep scheduling
- **Model Gossip:** Viral epidemic protocol (not data gossip)

## Quick Start

### Build & Run

```bash
cd examples/virtual_iot_network
cargo run --release
```

**Expected Output:**
```
🚀 QRES Virtual IoT Network Starting...
✓ Spawned 100 sensor nodes
✓ REST API listening on http://0.0.0.0:8080
✓ Consensus engine initialized

[Epoch 1] Consensus: 0.5012, Entropy: 0.12, Regime: Calm
[Epoch 2] Consensus: 0.5008, Entropy: 0.09, Regime: Calm
...
```

### Open Web Dashboard

Navigate to [http://localhost:8080](http://localhost:8080) in your browser to see:
- Real-time consensus convergence graph
- Per-node status (honest/byzantine/sleeping)
- Entropy timeline (Calm/PreStorm/Storm indicators)
- Bandwidth savings vs. federated learning baseline

### Inject Byzantine Nodes

While running, use the REST API:

```bash
# Inject 10 coordinated Byzantine attackers
curl -X POST http://localhost:8080/api/inject_byzantine \
  -H "Content-Type: application/json" \
  -d '{"count": 10, "bias": 0.9}'

# Trigger noise injection (Storm regime)
curl -X POST http://localhost:8080/api/inject_noise \
  -H "Content-Type: application/json" \
  -d '{"intensity": 0.5, "duration_sec": 300}'

# Query current state
curl http://localhost:8080/api/status
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  HTTP REST API (Warp)                                   │
│  http://localhost:8080                                  │
└──────────────────┬──────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────┐
│  Aggregator Node (qres_core)                            │
│  - Coordinate-wise trimmed mean                         │
│  - Reputation tracking                                  │
│  - Regime detection                                     │
└──────────────────┬──────────────────────────────────────┘
                   │
                   │ Model Gossip (Tokio channels)
                   │
           ┌───────┴────────┐
           │                │
           ▼                ▼
    ┌──────────┐       ┌──────────┐       ...  (×100)
    │ Sensor 1 │       │ Sensor 2 │
    │ (Honest) │       │ (Honest) │
    └──────────┘       └──────────┘
```

## Configuration

Edit [src/main.rs](src/main.rs) to adjust:

```rust
const NUM_NODES: usize = 100;
const NOISE_VARIANCE: f64 = 0.02;
const BYZANTINE_PERCENT: f64 = 0.0; // 0-30% tolerated
const REGIME_CALM_THRESHOLD: f64 = 0.15;
const TWC_INTERVAL_CALM: Duration = Duration::from_secs(14400); // 4 hours
```

## Performance

**Convergence Time:**
- Calm regime: ~30 epochs (~15 seconds with 500ms tick)
- Storm regime: ~50 epochs (~25 seconds, aggressive adaptation)

**Memory:**
- Per-node overhead: <1 KB (Q16.16 fixed-point state)
- Total (100 nodes): ~12 MB (including Tokio runtime)

**Bandwidth Savings:**
- Model gossip: 8 KB/day/node
- Federated learning baseline: 2.3 GB/day/node
- Compression ratio: ~287x (99.65% reduction)

## Verification

This example reproduces the v20.0 verification test:

1. **Convergence:** <30 epochs to consensus (verified)
2. **Byzantine Tolerance:** Drift <5% at 30% coordinated bias (verified)
3. **Scalability:** 100% success rate, 100-node swarm (verified)

See [docs/verification/QRES_V20_FINAL_VERIFICATION.md](../../docs/verification/QRES_V20_FINAL_VERIFICATION.md) for full results.

## Troubleshooting

### Port Already in Use

```bash
# Change port in src/main.rs:
const API_PORT: u16 = 8081;
```

### High CPU Usage

Reduce tick rate:
```rust
const TICK_INTERVAL: Duration = Duration::from_secs(1); // Was 500ms
```

### Node Failures

Check logs:
```bash
RUST_LOG=debug cargo run --release 2>&1 | tee network.log
```

## Next Steps

- **Modify Topology:** Edit `sensor_node.rs` to change network structure (mesh, star, tree)
- **Custom Predictors:** Implement `Predictor` trait for domain-specific models
- **Hardware Deployment:** Port to ESP32-C6 (see [docs/deployment/](../../docs/deployment/))
- **Add Visualization:** integrate with Bevy or web frontend (see [tools/swarm_sim/](../../tools/swarm_sim/))

## License

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE).
