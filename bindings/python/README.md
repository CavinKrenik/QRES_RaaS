# QRES Python Bindings

> **Resource-Aware Decentralized Node Mesh for Edge Computing**

High-performance Python bindings to `qres_core` (Rust) enabling deterministic Byzantine-tolerant compression, multimodal fusion, and P2P gossip for IoT and edge AI applications.

[![PyPI](https://img.shields.io/badge/PyPI-qres--raas-blue)](https://pypi.org/project/qres-raas/)
[![Python 3.8+](https://img.shields.io/badge/python-3.8+-blue.svg)](https://www.python.org/downloads/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](../../LICENSE-MIT)

---

## Features

- ✅ **Deterministic Compression:** Q16.16 fixed-point math (no floating-point non-determinism)
- ✅ **Byzantine Tolerance:** Coordinate-wise trimmed mean, reputation system (tolerates 30% attackers)
- ✅ **Multimodal Fusion:** TAAF (Temporal Attention-Guided Adaptive Fusion) with sparse spiking
- ✅ **Energy Efficiency:** TWT scheduling, regime-aware silence (82% radio sleep time)
- ✅ **No Heap Allocations:** `no_std` compatible core, <1 KB per-node overhead
- ✅ **Cross-Platform:** x86_64, ARM64, ARM32, RISC-V, WASM

**Performance:**
- 4.98x–31.8x compression vs. federated learning (dataset-dependent)
- ~99% bandwidth reduction (8 KB/day vs. 2.3 GB/day typical FL baseline)
- <30 epochs to consensus (100-node swarms, verified)

---

## Installation

### From PyPI (Recommended)

```bash
pip install qres-raas
```

### From Source (Development)

Requires [Rust 1.75+](https://rustup.rs/) and [maturin](https://github.com/PyO3/maturin):

```bash
# Clone repository
git clone https://github.com/CavinKrenik/QRES_RaaS.git
cd QRES_RaaS/bindings/python

# Install maturin
pip install maturin

# Build and install (development mode)
maturin develop --release

# Or build wheel for distribution
maturin build --release
```

### Dependencies

**Core (always required):**
```bash
pip install numpy scipy
```

**Optional (for specific features):**
```bash
# Lightweight features (CI testing)
pip install networkx gymnasium

# Full ML stack (local experimentation)
pip install torch sentence-transformers stable-baselines3 pandas matplotlib
```

Or install extras:
```bash
pip install qres-raas[ci]      # Lightweight
pip install qres-raas[ml]      # Full ML stack
```

---

## Quick Start

### Basic Compression

```python
from qres import QRES_API

# Initialize API
api = QRES_API(mode="hybrid")  # "hybrid" = adaptive regime detection

# Compress data
data = b"Sensor readings: temperature=23.5C, humidity=65%"
compressed = api.compress(data, usage_hint="iot")

# Decompress (deterministic)
decompressed = api.decompress(compressed)

assert data == decompressed
print(f"Compression ratio: {len(data) / len(compressed):.2f}x")
```

### Multimodal TAAF Fusion

```python
from qres.multimodal import TAAFPredictor, observe_multimodal
import numpy as np

# Initialize predictor
predictor = TAAFPredictor(
    num_modalities=3,
    fusion_mode="attention",
    spike_threshold=2.0
)

# Observe sensor streams (temperature, humidity, pressure)
modality_values = [23.5, 65.0, 1013.0]
prediction, attention_weights = observe_multimodal(predictor, modality_values)

print(f"Prediction: {prediction:.2f}")
print(f"Attention:  {attention_weights}")
```

### Swarm Node Participation

```python
from qres.swarm_cli import SwarmNode, SwarmConfig

# Configure node
config = SwarmConfig(
    node_id="sensor_42",
    listen_address="/ip4/0.0.0.0/tcp/0",
    bootstrap_peers=["/ip4/192.168.1.10/tcp/4001"],
    reputation_initial=0.8,
    regime="Calm"
)

# Launch node
node = SwarmNode(config)
print(f"PeerID: {node.peer_id}")
print(f"DID: did:qres:{node.did_suffix}")

# Participate in gossip
# ... (see examples/python/03_swarm_node.py)

node.shutdown()
```

---

## API Overview

### `qres.QRES_API`

Main compression interface.

**Methods:**
- `compress(data: bytes, usage_hint: str = "auto") -> bytes`
  - `usage_hint`: `"auto"`, `"iot"`, `"text"`, `"binary"`, `"semantic"`
- `decompress(compressed: bytes) -> bytes`

**Modes:**
- `"hybrid"`: Adaptive regime detection
- `"fixed"`: Fixed predictor
- `"multimodal"`: TAAF fusion

### `qres.multimodal`

Multimodal fusion and TAAF predictors.

**Classes:**
- `TAAFPredictor(num_modalities, fusion_mode, spike_threshold)` 
  - Temporal Attention-Guided Adaptive Fusion
- `LSTMPredictor(input_size, hidden_size, num_layers)`
  - LSTM-based compression model

**Functions:**
- `observe_multimodal(predictor, modality_values) -> (prediction, attention_weights)`

### `qres.swarm_cli`

P2P swarm node management.

**Classes:**
- `SwarmNode(config)` - libp2p-based gossip node
- `SwarmConfig(node_id, listen_address, bootstrap_peers, ...)`

### `qres.persistent`

Model persistence and serialization.

**Classes:**
- `ModelPersistence` - Trait for storage backends (disk, cloud, IPFS)

**Functions:**
- `save_model(model, path: str)`
- `load_model(path: str) -> model`

**Note:** `GeneStorage` is deprecated (v21.0), use `ModelPersistence` instead.

---

## Configuration

### Feature Flags

When building from source, you can enable/disable features:

```bash
# Minimal (no ML dependencies)
maturin develop --release --no-default-features --features python

# Full (all features)
maturin develop --release --features python,ml
```

### Environment Variables

- `QRES_LOG_LEVEL`: `DEBUG`, `INFO`, `WARN`, `ERROR` (default: `INFO`)
- `QRES_DATA_DIR`: Data directory for model checkpoints (default: `~/.qres`)

---

## Examples

See [examples/python/](../../examples/python/) for comprehensive examples:

1. **01_basic_compression.py** - Core API usage
2. **02_multimodal_taaf.py** - Multimodal fusion (v20.0)
3. **03_swarm_node.py** - P2P gossip protocol
4. **04_byzantine_defense.py** - Cartel detection (v20.0.1)
5. **05_regime_transitions.py** - Entropy-driven state machine
6. **06_persistent_state.py** - Non-volatile recovery (v18.0.0)

Run all:
```bash
cd ../../examples/python
python 01_basic_compression.py
# ... etc
```

---

## Performance & Benchmarks

### Compression Ratios (Verified v20.0)

| Dataset | Ratio vs. FL | Bandwidth Saved |
|---------|--------------|-----------------|
| SmoothSine | 31.8x | 96.9% |
| Wafer | 4.98x | 79.9% |
| ECG5000 | 4.98x | 79.9% |

### Throughput (x86_64, i7-10700K)

| Operation | Time | Throughput |
|-----------|------|------------|
| `compress()` | 100ns | 10M ops/sec |
| `decompress()` | 80ns | 12M ops/sec |
| TAAF `observe_multimodal()` | 500ns | 2M obs/sec |

### Memory

- Per-node overhead: <1 KB (Q16.16 fixed-point state)
- Python bindings: ~25 MB (includes maturin runtime)

---

## Troubleshooting

### Import Errors

```python
ImportError: No module named 'qres'
```

**Solution:** Install or rebuild bindings:
```bash
cd bindings/python
maturin develop --release
```

### Rust Compilation Errors

```
error: failed to compile `qres_core`
```

**Solution:** Update Rust toolchain:
```bash
rustup update stable
rustup default stable
```

### Missing Dependencies

```python
ModuleNotFoundError: No module named 'torch'
```

**Solution:** Install optional dependencies:
```bash
pip install qres-raas[ml]
```

Or skip ML features:
```bash
pip install qres-raas[ci]  # Lightweight
```

---

## Development

### Running Tests

Requires `pytest`:

```bash
pip install pytest pytest-cov
pytest tests/ -v
```

### Type Checking

Requires `mypy` and type stubs:

```bash
pip install mypy
mypy bindings/python/qres
```

### Building Documentation

```bash
pip install sphinx sphinx-rtd-theme
cd docs
make html
```

---

## Versioning

This package follows the core QRES version.

- **v21.0.0** (Current): Documentation restructure, INV-7 liveness
- **v20.0.1**: Adaptive defense, regime hysteresis
- **v20.0.0**: TAAF multimodal fusion, adaptive reputation exponent
- **v19.1.0**: TWT integration, power management

See [CHANGELOG.md](../../CHANGELOG.md) for full history.

---

## Contributing

Contributions welcome! See [CONTRIBUTING.md](../../docs/guides/CONTRIBUTING.md).

**Areas needing help:**
- Additional predictor implementations (Transformer, GRU)
- Cloud storage backends (S3, Azure Blob)
- Hardware-accelerated inference (ONNX Runtime, TensorRT)
- Jupyter notebook tutorials

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

## License

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
