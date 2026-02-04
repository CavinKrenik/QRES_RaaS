# Swarm Management and Deployment Guide

## Pre-Deployment Checklist

### Hardware Requirements

| Component | Specification | Notes |
|-----------|--------------|-------|
| MCU | ESP32-C6 (RISC-V) | Primary target; ARM Cortex-M4 also supported |
| RAM | >= 320 KB | 128 KB for model, 64 KB for ZK, 128 KB for stack/heap |
| Flash | >= 4 MB | Firmware + model weights + OTA partition |
| Solar Panel | >= 100 J/hr harvest | Required for indefinite operation in Calm regime |
| Battery | >= 1800 mAh @ 3.3V | 23,760 J capacity |
| WiFi | 802.11ax (WiFi 6) | Required for TWT (Target Wake Time) support |

### Firmware Build

```bash
# Build for ESP32-C6
cargo build --release --target riscv32imac-esp-espidf --features "esp32c6,std"

# Build WASM simulator
cargo build --release --target wasm32-unknown-unknown --features "wasm"

# Run cross-platform determinism check
cargo test --workspace -- test_deterministic
```

### Configuration Parameters

All configuration is in `config.toml` on the device flash:

```toml
[swarm]
node_id = "auto"           # Auto-generated ed25519 keypair
swarm_id = "field-001"     # Swarm identifier
max_peers = 20             # Maximum gossip neighbors

[reputation]
initial_trust = 0.5
ban_threshold = 0.2
ema_gamma = 0.05
zkp_reward = 0.02
drift_penalty = 0.08
zkp_failure_penalty = 0.15

[aggregation]
mode = "weighted_trimmed_mean"
trim_f = 2

[regime]
calm_twt_s = 14400         # 4 hours
pre_storm_twt_s = 600      # 10 minutes
storm_twt_s = 30            # 30 seconds
entropy_threshold = 2.0
min_trusted_confirmations = 3
min_vote_reputation = 0.8

[zk]
audit_interval = 50
response_deadline = 5

[energy]
solar_panel_j_per_hr = 100
battery_capacity_j = 23760
brownout_threshold_pct = 5   # Enter safe mode below 5%
```

## Deployment Procedures

### Initial Bootstrap

1. **Flash firmware** to all nodes via USB or OTA
2. **Generate keypairs:** Each node generates ed25519 keypair on first boot. Public key becomes the PeerId.
3. **Seed the swarm:** First 3 nodes form the bootstrap quorum. They exchange public keys via local broadcast.
4. **Join procedure:** Subsequent nodes discover the swarm via mDNS and request membership. Existing nodes add the new node at `initial_trust = 0.5`.

### Scaling: Adding Nodes

When adding new nodes to a running swarm:

1. New node boots and announces via mDNS
2. Existing nodes add it with R=0.5 (default trust)
3. New node must prove itself over ~15 rounds (R reaches 0.8 via ZKP rewards)
4. Until R >= 0.4, the node's updates have minimal weight in aggregation
5. Until R >= 0.8, the node cannot vote on regime transitions

**No configuration changes needed on existing nodes.** The reputation system handles trust bootstrapping automatically.

### Scaling: Removing Nodes

Nodes can leave gracefully or crash:

- **Graceful departure:** Node broadcasts a leave message. Peers remove it from their peer list.
- **Crash/power loss:** Node stops participating. After `3 * twt_interval` of silence, peers mark it as inactive (not banned, just absent). If it returns, it resumes with its last known reputation.
- **Permanent removal:** Operator can broadcast a revocation message signed by the swarm admin key (optional).

### Geographic Distribution

For multi-site deployments:

```
Site A (10 nodes)  <--gossip bridge-->  Site B (10 nodes)
       |                                       |
  Local WiFi mesh                        Local WiFi mesh
```

- Each site forms a local gossip mesh
- A "bridge node" at each site relays consensus to the other site
- Cross-site latency is acceptable because QRES consensus is asynchronous (no real-time coordination needed)

## Monitoring

### Health Metrics

Each node exposes the following metrics via MQTT or serial console:

| Metric | Description | Alert Threshold |
|--------|-------------|----------------|
| `reputation` | Own reputation score | < 0.4 (investigate) |
| `battery_pct` | Battery level | < 10% (brownout risk) |
| `active_peers` | Number of active peers | < 3 (quorum risk) |
| `consensus_drift` | L2 drift from last consensus | > 0.1 (model divergence) |
| `banned_peers` | Number of banned peers | > n/3 (possible attack) |
| `regime` | Current regime (calm/pre-storm/storm) | Storm > 1 hour (energy drain) |
| `zk_audit_status` | Last audit result | Any failure (investigate) |

### Operator Dashboard

```
+----------------------------------+
| QRES Swarm: field-001            |
| Active: 18/20  Banned: 2         |
| Regime: Calm   TWT: 4h           |
| Consensus Drift: 0.012           |
| Battery (avg): 94%               |
| Last Audit: Round 150 - PASS     |
+----------------------------------+
```

## Troubleshooting

### Node Not Joining Swarm

1. Verify WiFi connectivity: `ping <bootstrap_node_ip>`
2. Check mDNS: node should broadcast `_qres._tcp.local`
3. Verify firmware version matches swarm (version mismatch → incompatible ZK proofs)
4. Check clock synchronization: TWT requires ~1s accuracy

### Node Wrongly Banned

Symptoms: Honest node's reputation drops below 0.2

1. Check node's sensor readings: faulty sensor → bad model updates → legitimate drift detection
2. Check firmware version: outdated firmware may compute different Q16.16 results
3. Check solar panel: insufficient power → missed rounds → reputation decay
4. Recovery: Reflash firmware, reboot. Node re-enters at R=0.5.

### Persistent High Drift

Symptoms: Consensus drift > 0.05 for > 20 rounds

1. Check `banned_peers` count: if < expected Byzantine count, reputation hasn't caught up yet
2. Check for mimicry attackers: low drift but persistent (Class B). Trimming bounds this.
3. Check model complexity: if DIM > 50, convergence is slower (Theorem 3: $T \propto d$)
4. Increase `trim_f` if adversary fraction is higher than expected

### Energy Issues

Symptoms: Battery dropping faster than expected

1. Verify regime: Storm regime consumes 207x more than Calm
2. Check solar harvest: Winter/cloudy conditions reduce harvest
3. If in Storm: verify entropy source (is the environment actually volatile?)
4. Auto-downgrade: Node should automatically reduce to Calm when battery < 20%

## Recovery Procedures

### Lamarckian Recovery (Model Checkpoint)

QRES supports "Lamarckian recovery" — when a node rejoins after extended absence, it inherits the swarm's current consensus model rather than starting from scratch.

**Recovery protocol:**

1. Rejoining node broadcasts `RECOVERY_REQUEST` with its last known consensus hash
2. Nearest active peer responds with current consensus weights (signed)
3. Rejoining node verifies signature and adopts the weights as its starting point
4. Reputation resets to `initial_trust = 0.5` (trust must be re-earned)

**Why "Lamarckian":** Unlike biological evolution where offspring start from scratch, QRES nodes inherit acquired knowledge (the trained model) from the swarm. This is analogous to Lamarckian inheritance.

### Disaster Recovery (Swarm Reset)

If > 2/3 of nodes are compromised or offline:

1. Operator broadcasts `SWARM_RESET` signed by admin key
2. All nodes reset reputation to `initial_trust = 0.5`
3. All nodes keep their current model weights (no data loss)
4. Bootstrap quorum re-forms from the first 3 responding nodes
5. Normal operation resumes

**When to use:** Only when the swarm is in an unrecoverable state (e.g., all nodes banned each other due to firmware bug, or after physical node replacement).

### Firmware Update (OTA)

1. Operator pushes new firmware to OTA server
2. Nodes check for updates during Calm regime wake windows
3. Download occurs during active window; install during next sleep cycle
4. After reboot, node re-announces to swarm
5. Reputation is preserved across firmware updates (stored in NVS)

**Safety:** OTA images are signed by the build server. Nodes verify the signature before installing. This prevents supply-chain attacks via compromised OTA.
