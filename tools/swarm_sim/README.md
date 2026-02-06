# Legacy Swarm Simulator

**Status:** 🟡 Frozen (No active development)  
**Last Updated:** v15.0 (2023)  
**Purpose:** Historical reference & paper reproducibility

---

## Overview

This is the **legacy** Python-based swarm simulator used in early QRES research (2023-2024). It has been **superseded** by:

1. **`qres_sim` crate** (Rust) - Production-grade simulation framework
2. **`examples/virtual_iot_network/`** - 100-node demo with Byzantine injection

**Do not use this for new development.** It is maintained solely for:
- Reproducing results from ICCV 2023 and CVPR 2024 papers
- Historical comparison with modern implementation
- Academic reproducibility requirements

---

## Why Is This Here?

### Research Lineage

The legacy simulator was the proof-of-concept for:
- **Viral epidemic protocol** (now in `qres_swarm`)
- **Reputation-weighted aggregation** (now in `qres_core::byzantine`)
- **Regime-aware compression** (now `RegimeDetector`)

### Papers Using Legacy Code

| Paper | Conference | Code Version |
|-------|-----------|--------------|
| "Resource-Aware Swarm Learning" | ICCV 2023 | `swarm_sim` v0.3 |
| "Byzantine-Tolerant Edge AI" | CVPR 2024 | `swarm_sim` v0.5 |

**Reproducibility Note:** To reproduce paper results, check out git tags:
```bash
# ICCV 2023 experiments
git checkout v0.3.0-iccv2023
cd tools/swarm_sim
python simulate.py --config configs/iccv_fig3.toml

# CVPR 2024 experiments  
git checkout v0.5.2-cvpr2024
cd tools/swarm_sim
python simulate.py --config configs/cvpr_byzantine.toml
```

---

## Architecture

```
tools/swarm_sim/
├── src/
│   └── main.rs          # Rust entry point (uses deprecated GeneStorage)
├── simulate.py          # Python simulator (legacy)
├── configs/             # Experiment configurations
│   ├── iccv_fig3.toml
│   └── cvpr_byzantine.toml
└── README.md            # This file
```

### Key Components

| File | Purpose | Current Equivalent |
|------|---------|-------------------|
| `simulate.py` | Main simulation loop | `examples/virtual_iot_network/` |
| `src/main.rs` | Rust visualization tool | `qres_sim` crate |
| `configs/*.toml` | Experiment configs | `qres_sim::Config` |

---

## Known Limitations

### 1. Performance
- **Legacy:** ~10 nodes max (Python overhead)
- **Modern:** 1000+ nodes (`qres_sim` in Rust)

### 2. Features
- ❌ No TAAF support (pre-v16.0)
- ❌ No adaptive Byzantine defense (uses fixed trimmed mean)
- ❌ No regime hysteresis (only binary Calm/Storm)
- ❌ No TWT energy modeling

### 3. Accuracy
- **Legacy:** Simplified fixed-point simulation (8-bit quantization)
- **Modern:** Full Q16.16 deterministic compression

### 4. Maintenance
- ❌ Not tested on Python 3.12+
- ❌ Dependencies frozen (see `requirements_legacy.txt`)
- ❌ Known bugs not fixed (e.g., race condition in `viral_broadcast()`)

---

## Migration Guide

### From Legacy Simulator to Modern Code

#### 1. Basic Simulation

**Legacy:**
```bash
cd tools/swarm_sim
python simulate.py --nodes 10 --byzantine 2 --steps 100
```

**Modern:**
```bash
cd examples/virtual_iot_network
cargo run --release -- \
  --nodes 100 \
  --byzantine-count 10 \
  --duration 60s
```

#### 2. Custom Experiments

**Legacy:**
```python
# tools/swarm_sim/simulate.py
from swarm import SwarmNode, ViralProtocol

nodes = [SwarmNode(id=i) for i in range(10)]
protocol = ViralProtocol(infection_rate=0.7)

for step in range(100):
    for node in nodes:
        node.update(protocol)
        node.aggregate()
```

**Modern:**
```rust
// Using qres_sim crate
use qres_sim::{Simulation, SwarmConfig};

let config = SwarmConfig {
    num_nodes: 10,
    byzantine_ratio: 0.2,
    gossip_fanout: 7,
    ..Default::default()
};

let mut sim = Simulation::new(config);
for step in 0..100 {
    sim.step();
    let metrics = sim.collect_metrics();
    println!("Step {}: consensus_error={:.4}", step, metrics.consensus_error);
}
```

#### 3. Visualization

**Legacy:**
```bash
# Generate legacy plots
python tools/swarm_sim/visualize.py --results results.json
```

**Modern:**
```bash
# Generate modern plots with better styling
cargo run -p qres_sim --example visualize -- \
  --results results.msgpack \
  --format pdf
```

---

## Comparison Matrix

| Feature | Legacy `swarm_sim` | Modern `qres_sim` |
|---------|-------------------|-------------------|
| **Language** | Python 3.8 | Rust 1.75+ |
| **Max Nodes** | ~10 | 10,000+ |
| **Performance** | ~1 step/sec | ~1000 steps/sec |
| **TAAF** | ❌ | ✅ |
| **Byzantine Defense** | Fixed trimmed mean | Adaptive + Grubbs' |
| **Regime Detection** | Binary threshold | Hysteresis + history |
| **Energy Model** | None | TWT-aware |
| **Determinism** | ❌ (float rounding) | ✅ (Q16.16) |
| **Tests** | 12 unit tests | 156 unit + 56 integration |
| **Documentation** | Minimal docstrings | Full rustdoc |
| **Active Development** | ❌ Frozen | ✅ Maintained |

---

## When To Use Legacy Code

### ✅ Use Legacy Code If:
1. **Reproducing paper results** (ICCV 2023, CVPR 2024)
2. **Comparing with baseline** (showing modern improvements)
3. **Understanding historical design decisions** (git archaeology)

### ❌ Don't Use Legacy Code If:
1. **Starting new research** → Use `qres_sim` crate
2. **Production deployment** → Use `qres_core` + `qres_swarm`
3. **Teaching/tutorials** → Use `examples/virtual_iot_network/`
4. **Benchmarking current performance** → Use `cargo bench`

---

## Running Legacy Code (For Reproducibility)

### Prerequisites

```bash
# Python 3.8-3.10 only (not tested on 3.11+)
python3.8 -m venv venv_legacy
source venv_legacy/bin/activate  # Windows: venv_legacy\Scripts\activate
pip install -r tools/swarm_sim/requirements_legacy.txt
```

### Run ICCV 2023 Experiment

```bash
cd tools/swarm_sim
python simulate.py --config configs/iccv_fig3.toml

# Expected output: results/iccv_fig3_convergence.png
```

### Run CVPR 2024 Byzantine Experiment

```bash
cd tools/swarm_sim  
python simulate.py --config configs/cvpr_byzantine.toml

# Expected output: results/cvpr_byzantine_detection.png
```

### Troubleshooting

**Issue:** `ImportError: No module named 'msgpack'`  
**Fix:** `pip install msgpack==1.0.3` (specific version for legacy code)

**Issue:** `RuntimeError: viral_broadcast() race condition`  
**Fix:** Known bug. Add `--single-threaded` flag.

**Issue:** Different results than paper  
**Fix:** Use exact Python version (3.8.10) and dependencies from `requirements_legacy.txt`

---

## Future Plans

**v22.0 (2026 Q3):** Move to `docs/archive/legacy/`

This code will be:
1. Archived in `docs/archive/legacy/swarm_sim/`
2. Marked as unsupported
3. Removed from main `tools/` directory
4. Still accessible via git tags for reproducibility

**Rationale:** Clean up main codebase while preserving historical reference.

---

## FAQ

**Q: Can I contribute fixes to legacy code?**  
A: No. Legacy code is frozen. Contribute to `qres_sim` crate instead.

**Q: Why not delete it?**  
A: Academic reproducibility requirements (papers cite this code).

**Q: Is there a migration script?**  
A: No automated migration. Use modern APIs directly (they're better designed).

**Q: Can I use legacy and modern code together?**  
A: Not recommended. APIs are incompatible.

---

## See Also

- [docs/reference/DEPRECATED.md](../../docs/reference/DEPRECATED.md) - All deprecated APIs
- [examples/virtual_iot_network/](../../examples/virtual_iot_network/) - Modern demo
- [qres_sim crate](../../crates/qres_sim/) - Production simulator
- [CHANGELOG.md](../../CHANGELOG.md) - Version history

---

## Contact

**Questions about reproducing paper results?**  
Open issue with `reproducibility` label: https://github.com/your-org/qres/issues/new?labels=reproducibility

**Found a bug in modern code?**  
That's where we fix bugs! Open regular issue.

---

**Frozen:** v15.0 (October 2023)  
**Status:** Read-only (no PRs accepted)  
**Archive Date:** v22.0 (planned 2026 Q3)
