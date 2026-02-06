# QRES Python Examples

Hands-on examples demonstrating QRES v21.0 features using the Python API.

## Installation

### Option 1: From Source (Recommended for Development)

```bash
cd bindings/python
maturin develop --release
```

### Option 2: From PyPI (Coming Soon)

```bash
pip install qres-raas
```

### Dependencies

```bash
# Required
pip install numpy scipy

# Optional (for specific examples)
pip install matplotlib  # For visualization (05_regime_transitions.py)
```

---

## Examples Overview

| Example | Feature | v21 Milestone | Run Time |
|---------|---------|---------------|----------|
| **01_basic_compression.py** | Core compress/decompress API, determinism | v21.0 | ~1s |
| **02_multimodal_taaf.py** | TAAF fusion, event-driven spiking | v20.0 | ~2s |
| **03_swarm_node.py** | P2P swarm, viral gossip, reputation | v20.0 | ~3s |
| **04_byzantine_defense.py** | Adaptive aggregation, cartel detection | v20.0.1 | ~2s |
| **05_regime_transitions.py** | Entropy-driven state machine, hysteresis | v20.0.1 | ~3s |
| **06_persistent_state.py** | Non-volatile model recovery | v18.0.0 | ~2s |

**Total:** 6 examples, ~13 seconds to run sequentially

---

## Quick Start

### Run All Examples

```bash
cd examples/python

# Run individually
python 01_basic_compression.py
python 02_multimodal_taaf.py
python 03_swarm_node.py
python 04_byzantine_defense.py
python 05_regime_transitions.py
python 06_persistent_state.py

# Or batch run (Unix/Linux/macOS)
for f in 0*.py; do python $f; done

# Batch run (Windows PowerShell)
Get-ChildItem -Filter "0*.py" | ForEach-Object { python $_.Name }
```

### Run Single Example

```bash
python 01_basic_compression.py
```

**Expected Output:**
```
============================================================
QRES v21.0 - Basic Compression Example
============================================================
✓ QRES API initialized (mode=hybrid)

Example 1: Text Data
------------------------------------------------------------
Original size:    460 bytes
Compressed size:  120 bytes
Compression ratio: 3.83x
Bandwidth saved:  340 bytes (73.9%)
✓ Deterministic decompression verified
...
```

---

## Example Details

### 01_basic_compression.py

**Learn:** Core QRES API for compression and decompression

**Key Concepts:**
- `QRES_API(mode="hybrid")` initialization
- `compress(data, usage_hint)` with hints: `"text"`, `"iot"`, `"binary"`, `"auto"`
- `decompress(compressed)` deterministic recovery
- Compression ratio analysis
- Determinism verification (same input → same output)

**Expected Results:**
- Text compression: ~3-5x ratio
- IoT sensor data: ~2-4x ratio
- Determinism: 100% identical across runs

---

### 02_multimodal_taaf.py

**Learn:** Temporal Attention-Guided Adaptive Fusion (TAAF)

**Key Concepts:**
- Cross-modal sensor fusion (temperature, humidity, pressure)
- Event-driven sparse spiking (Welford's online variance)
- Attention weight calculation (softmax normalization, Q16.16 fixed-point)
- 3.6% RMSE improvement over unimodal

**Expected Results:**
- Average residual: ~0.02-0.05
- Spike events: ~10-20% (60% energy savings vs. full attention)
- Attention weights dynamically adjust to changing modalities

**Theory:**
See [docs/reference/ARCHITECTURE.md](../../docs/reference/ARCHITECTURE.md) Section 3 for TAAF pipeline diagram.

---

### 03_swarm_node.py

**Learn:** P2P swarm participation and gossip protocol

**Key Concepts:**
- `SwarmNode` initialization with libp2p
- W3C DID generation (`did:qres:<hex>`)
- Viral epidemic AD-SGD protocol
- Infection criteria: `accuracy_delta > cure_threshold`
- Epidemic priority: `residual_error × accuracy_delta × reputation`
- Adaptive reputation exponent (2.0/3.0/3.5 by swarm size)
- Influence cap: `rep³ × 0.8`

**Expected Results:**
- Node successfully joins swarm
- Gossip updates propagate based on epidemic priority
- Reputation evolves from 0.8 → 0.9+ (with good contributions)

**Production Deployment:**
See [examples/virtual_iot_network/](../virtual_iot_network/) for full 100-node demo with REST API.

---

### 04_byzantine_defense.py

**Learn:** Byzantine-tolerant aggregation and cartel detection

**Key Concepts:**
- **Calm regime:** Reputation-weighted aggregation (13.8% overhead reduction)
- **Storm regime:** Coordinate-wise trimmed mean (trim 20% per dimension)
- **Stochastic auditing:** 3% ZK proof verification sample rate
- **Class C detection:** Grubbs' test (α=0.01) for outlier cartels
- **Verification:** 100% detection, 0% false positives (v20.0.1)

**Expected Results:**
- Drift <5% at 30% Byzantine attackers (verified tolerance)
- 10/10 Byzantine nodes detected (simulated cartel)
- 0/390 honest nodes falsely banned
- Bandwidth overhead: 2.0% (stochastic auditing)

**Security Analysis:**
See [docs/security/CLASS_C_DEFENSE.md](../../docs/security/CLASS_C_DEFENSE.md) for full protocol specification.

---

### 05_regime_transitions.py

**Learn:** Entropy-driven regime detection and state machine

**Key Concepts:**
- **Entropy calculation:** `|actual - predicted| / range` (3-point moving average)
- **Entropy derivative:** `(entropy[t] - entropy[t-2]) / 2Δt`
- **Thresholds:**
  - θ₁ = 0.15 (Calm → PreStorm derivative threshold)
  - θ₂ = 0.45 (PreStorm → Storm raw entropy critical)
  - θ₃ = 0.30 (Storm → Calm recovery threshold)
- **Hysteresis:** Asymmetric confirmation (96.9% false-positive reduction)
- **TWT intervals:** Calm=4h, PreStorm=10m, Storm=30s

**Expected Results:**
- ~87% time in Calm regime (stable workloads)
- 3-5 total regime transitions (vs. ~100 without hysteresis)
- Storm duration: ~12 minutes (during noise injection)
- >80% sleep time achieved

**Tuning:**
See [docs/adaptive/META_TUNING.md](../../docs/adaptive/META_TUNING.md) for threshold calibration guide.

---

### 06_persistent_state.py

**Learn:** Non-volatile model persistence and reboot recovery

**Key Concepts:**
- **ModelPersistence trait** (replaces deprecated `GeneStorage`)
- Model parameter serialization (JSON/binary)
- Storage backends: disk, cloud, IPFS
- Error delta analysis: 4% tolerance (v18.0.0 verification)
- Zero catastrophic knowledge loss

**Expected Results:**
- 8 checkpoints saved before reboot
- Model recovered with <4% error delta
- Training resumes seamlessly post-recovery
- Final accuracy matches pre-reboot trajectory

**Migration:**
`GeneStorage` deprecated in v21.0.0, removed in v22.0.0. Use `ModelPersistence` instead.

---

## Troubleshooting

### Import Errors

```python
ImportError: No module named 'qres'
```

**Solution:**
```bash
cd bindings/python
maturin develop --release
```

### API Not Available

```python
AttributeError: module 'qres' has no attribute 'TAAFPredictor'
```

**Explanation:** Some APIs are in development. Examples gracefully degrade:
```python
try:
    from qres.multimodal import TAAFPredictor
except ImportError:
    print("⚠️ TAAF not available, using fallback")
```

### Data Files Missing

```python
FileNotFoundError: data/jena.csv
```

**Solution:** Examples use synthetic data (no external files required). If using custom datasets:
```bash
# Optional: Download real IoT datasets
cd data
wget https://example.com/datasets/jena.csv
```

---

## Running Tests on Examples

Ensure examples work correctly:

```bash
# Install pytest
pip install pytest

# Add test wrapper (optional)
pytest --doctest-modules examples/python/
```

---

## Performance Benchmarks

Measured on i7-10700K, Ubuntu 22.04, Python 3.11:

| Example | Runtime | Peak Memory |
|---------|---------|-------------|
| 01_basic_compression.py | 0.8s | 25 MB |
| 02_multimodal_taaf.py | 1.2s | 35 MB |
| 03_swarm_node.py | 2.5s | 30 MB |
| 04_byzantine_defense.py | 1.8s | 40 MB |
| 05_regime_transitions.py | 2.1s | 32 MB |
| 06_persistent_state.py | 1.5s | 28 MB |

**Total:** ~10 seconds, <50 MB peak

---

## Next Steps

### Explore Rust Examples

See [examples/rust/](../rust/) for:
- Custom predictor implementation
- `no_std` embedded usage
- Performance-critical paths

### Run Full Swarm Demo

```bash
cd examples/virtual_iot_network
cargo run --release
# Open: http://localhost:8080
```

100-node IoT mesh with:
- Real-time convergence visualization
- Byzantine node injection
- Regime transition monitoring

### Read API Reference

Complete documentation: [docs/reference/API_REFERENCE.md](../../docs/reference/API_REFERENCE.md)

Python-specific APIs:
- `qres.QRES_API` - Main compression interface
- `qres.multimodal.TAAFPredictor` - Multimodal fusion
- `qres.swarm_cli.SwarmNode` - P2P node management
- `qres.persistent.ModelPersistence` - State persistence

### Dive into Theory

Mathematical foundations:
- [RaaS_Extras/docs/theory/THEORY.md](../../../RaaS_Extras/docs/theory/THEORY.md)
- [RaaS_Extras/docs/theory/PAC_REPUTATION_BOUNDS.md](../../../RaaS_Extras/docs/theory/PAC_REPUTATION_BOUNDS.md)
- [docs/verification/FORMAL_SPEC.md](../../docs/verification/FORMAL_SPEC.md) (TLA+)

---

## Contributing

Found a bug or want to add an example?

1. Read [docs/guides/CONTRIBUTING.md](../../docs/guides/CONTRIBUTING.md)
2. Check [docs/status/TECHNICAL_DEBT.md](../../docs/status/TECHNICAL_DEBT.md)
3. Open an issue or PR

**Example Ideas:**
- Real-time sensor ingestion (MQTT/CoAP)
- Custom predictor for domain-specific data
- Integration with TensorFlow/PyTorch models
- Cloud deployment (AWS Lambda, Azure Functions)

---

## License

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.

---

## Citation

```bibtex
@software{qres2026,
  author = {Krenik, Cavin},
  title = {QRES: Resource-Aware Agentic Swarm},
  url = {https://github.com/CavinKrenik/QRES_RaaS},
  doi = {10.5281/zenodo.18474976},
  year = {2026}
}
```

**Paper:** [RaaS: Resource-Aware Agentic Swarm](https://doi.org/10.5281/zenodo.18474976)

---

**Questions?** Open an issue or see [docs/INDEX.md](../../docs/INDEX.md) for full documentation index.
