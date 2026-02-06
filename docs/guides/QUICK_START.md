# QRES Quick Start Guide

**Get up and running with QRES in 10 minutes**

This hands-on tutorial will guide you through:
1. Installing QRES (Rust + Python)
2. Running your first compression
3. Exploring the 100-node IoT demo
4. Understanding the results

**Time required:** ~10 minutes  
**Prerequisites:** Basic command-line familiarity

---

## Step 1: Install Prerequisites (2 minutes)

### Rust Installation

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow prompts, then reload shell
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should show 1.75+ or higher
```

**Windows:** Download from [rustup.rs](https://rustup.rs/)

### Python Installation (Optional)

For Python bindings:

```bash
# Check Python version (need 3.8+)
python --version

# Install maturin
pip install maturin
```

---

## Step 2: Clone & Build (3 minutes)

```bash
# Clone repository
git clone https://github.com/CavinKrenik/QRES_RaaS.git
cd QRES_RaaS

# Build core (release mode for performance)
cargo build --release

# Run quick tests to verify
cargo test -p qres_core --features std --release
```

**Expected output:**
```
running 134 tests
...
test result: ok. 134 passed; 0 failed; 0 ignored
```

---

## Step 3: Your First Compression (2 minutes)

### Option A: Rust

Create `hello_qres.rs`:

```rust
use qres_core::{compress, decompress};

fn main() {
    let data = b"Hello, QRES! This is deterministic compression.";
    
    // Compress
    let compressed = compress(data).expect("Compression failed");
    println!("Original:   {} bytes", data.len());
    println!("Compressed: {} bytes", compressed.len());
    println!("Ratio:      {:.2}x", data.len() as f32 / compressed.len() as f32);
    
    // Decompress
    let decompressed = decompress(&compressed).expect("Decompression failed");
    assert_eq!(data.as_slice(), decompressed.as_slice());
    println!("✓ Verified: Decompression successful!");
}
```

Run:
```bash
rustc hello_qres.rs --edition 2021 -L target/release/deps -l qres_core
./hello_qres
```

### Option B: Python

First, build Python bindings:

```bash
cd bindings/python
maturin develop --release
cd ../..
```

Then run Python:

```python
from qres import QRES_API

api = QRES_API(mode="hybrid")

data = b"Hello, QRES! This is deterministic compression."
compressed = api.compress(data, usage_hint="text")

print(f"Original:   {len(data)} bytes")
print(f"Compressed: {len(compressed)} bytes")
print(f"Ratio:      {len(data) / len(compressed):.2f}x")

decompressed = api.decompress(compressed)
assert data == decompressed
print("✓ Verified: Decompression successful!")
```

**Expected output:**
```
Original:   48 bytes
Compressed: 12 bytes
Ratio:      4.00x
✓ Verified: Decompression successful!
```

---

## Step 4: Run the 100-Node IoT Demo (3 minutes)

This demonstrates real-world usage: a sensor mesh with Byzantine fault tolerance.

```bash
cd examples/virtual_iot_network
cargo run --release
```

**What you'll see:**
```
🚀 QRES Virtual IoT Network Starting...
✓ Spawned 100 sensor nodes
✓ REST API listening on http://0.0.0.0:8080
✓ Consensus engine initialized

[Epoch 1] Consensus: 0.5012, Entropy: 0.12, Regime: Calm, Nodes: 100
[Epoch 2] Consensus: 0.5008, Entropy: 0.09, Regime: Calm, Nodes: 100
[Epoch 3] Consensus: 0.5005, Entropy: 0.07, Regime: Calm, Nodes: 100
...
[Epoch 28] Consensus: 0.5000, Entropy: 0.02, Regime: Calm, Nodes: 100
✓ Convergence achieved in 28 epochs!
```

### Open Web Dashboard

While the demo runs, open your browser:

**URL:** [http://localhost:8080](http://localhost:8080)

You'll see:
- **Real-time graph:** Consensus convergence
- **Node status:** Honest (green), Byzantine (red), Sleeping (gray)
- **Entropy timeline:** Calm/PreStorm/Storm regime indicators
- **Bandwidth metrics:** Compression ratio vs. federated learning

### Inject a Byzantine Attack

In another terminal:

```bash
curl -X POST http://localhost:8080/api/inject_byzantine \
  -H "Content-Type: application/json" \
  -d '{"count": 10, "bias": 0.9}'
```

**Observe:**
1. Graph shows 10 red nodes appear
2. Consensus temporarily drifts (but stays <5%)
3. System detects cartel (usually within ~80 rounds)
4. Byzantine nodes are isolated
5. Consensus recovers

**This demonstrates:**
- ✅ Byzantine tolerance (30% attackers tolerated)
- ✅ Adaptive detection (Class C cartel identification)
- ✅ Self-healing (automatic recovery)

---

## Understanding the Results

### What Just Happened?

**Traditional Federated Learning:**
- Each node sends raw data or full model (GBs)
- Central aggregator merges updates
- Bandwidth: ~2.3 GB/day per node

**QRES Approach:**
- Nodes gossip small model bytecode updates (KBs)
- Deterministic rematerialization: receivers reconstruct data
- Bandwidth: ~8 KB/day per node
- **Result:** 4.98x-31.8x compression (dataset-dependent), ~99% bandwidth reduction baseline

### Key Metrics Explained

| Metric | Meaning | Good Value |
|--------|---------|-----------|
| **Consensus** | Average model prediction | Convergence: < 0.01 drift |
| **Entropy** | Prediction uncertainty | Calm: <0.15, Storm: >0.45 |
| **Regime** | System state | Calm: 87% of time |
| **Epochs to Convergence** | Rounds until consensus | <30 epochs (verified) |
| **Byzantine Tolerance** | Drift under attack | <5% at 30% attackers |

### Performance Numbers (Your Machine)

The demo prints performance stats at the end:

```
Performance Summary:
  Total runtime:        45.2 seconds
  Consensus rounds:     28
  Bandwidth per node:   284 bytes (vs 2.1 MB federated)
  Compression ratio:    7,394x
  Memory per node:      872 bytes
  Byzantine detected:   10/10 (100%)
  False positives:      0/390 (0%)
```

---

## Next Steps

### Explore Examples

**Python Examples** (6 code samples):
```bash
cd examples/python
python 01_basic_compression.py    # API basics
python 02_multimodal_taaf.py      # Sensor fusion
python 03_swarm_node.py           # P2P gossip
python 04_byzantine_defense.py    # Attack simulation
python 05_regime_transitions.py   # State machine
python 06_persistent_state.py     # Storage & recovery
```

**Rust Examples:**
```bash
cd examples/rust/01_basic_compression
cargo run --release

cd examples/rust/02_custom_predictor
cargo run --release
```

### Read Documentation

**Start here:**
1. [Architecture Overview](../reference/ARCHITECTURE.md) - System diagrams
2. [API Reference](../reference/API_REFERENCE.md) - Complete API docs
3. [API Cookbook](API_COOKBOOK.md) - Common recipes

**Deep dives:**
- [TAAF Multimodal Fusion](../verification/QRES_V20_FINAL_VERIFICATION.md)
- [Byzantine Defense](../security/CLASS_C_DEFENSE.md)
- [TWT Power Management](../power/TWT_INTEGRATION.md)
- [Formal Verification](../verification/FORMAL_SPEC.md)

### Hardware Deployment

**ESP32-C6 (RISC-V):**
```bash
rustup target add riscv32imc-unknown-none-elf
cargo build -p qres_core --target riscv32imc-unknown-none-elf \
  --no-default-features --release
```

**Raspberry Pi:**
```bash
# Cross-compile for ARM64
cross build --target aarch64-unknown-linux-gnu --release
scp target/aarch64-unknown-linux-gnu/release/qres_daemon pi@192.168.1.10:~
ssh pi@192.168.1.10 ./qres_daemon
```

### Customize & Extend

**Implement Custom Predictor:**

See [examples/rust/02_custom_predictor/](../../examples/rust/02_custom_predictor/)

```rust
use qres_core::Predictor;

struct MyPredictor {
    // Your model here
}

impl Predictor for MyPredictor {
    fn predict(&self, input: &[I16F16]) -> I16F16 {
        // Your prediction logic
    }
}
```

**Integrate with Existing ML:**

```python
import torch
from qres import QRES_API

# Train your PyTorch model
model = torch.nn.Linear(10, 1)
# ... training ...

# Use QRES for deployment compression
api = QRES_API(mode="hybrid")
model_bytes = serialize(model)  # Your serialization
compressed = api.compress(model_bytes, usage_hint="binary")

# Deploy compressed model to edge devices
```

---

## Troubleshooting

### Build Errors

**Error:** `error: could not compile 'qres_core'`

**Solution:**
```bash
# Update Rust
rustup update stable

# Clean build
cargo clean
cargo build --release
```

### Python Import Errors

**Error:** `ImportError: No module named 'qres'`

**Solution:**
```bash
cd bindings/python
maturin develop --release --force
```

### Port Already in Use

**Error:** `Address already in use (os error 98)`

**Solution:**
```bash
# Find process using port 8080
lsof -i :8080  # macOS/Linux
netstat -ano | findstr :8080  # Windows

# Kill process or change port in src/main.rs:
const API_PORT: u16 = 8081;
```

### High CPU Usage

If the demo uses too much CPU:

```bash
# Reduce node count
# Edit examples/virtual_iot_network/src/main.rs:
const NUM_NODES: usize = 20;  // Was 100

# Increase tick interval
const TICK_INTERVAL: Duration = Duration::from_secs(1);  # Was 500ms
```

---

## FAQ

**Q: How does QRES achieve determinism?**  
A: All math uses Q16.16 fixed-point arithmetic (no floats). Same input → same output across x86/ARM/RISC-V.

**Q: What's the performance vs. compression tradeoff?**  
A: Compression is <100ns per operation. Bandwidth savings (4.98x-31.8x) far outweigh CPU cost.

**Q: Can I use QRES with TensorFlow/PyTorch models?**  
A: Yes! Serialize your model, compress with QRES, deploy to edge. See [API Cookbook](API_COOKBOOK.md) for examples.

**Q: Is QRES production-ready?**  
A: Core is production-grade (212 tests, formal verification started). P2P layer is Beta. ESP32 deployment is experimental.

**Q: How do I contribute?**  
A: See [CONTRIBUTING.md](CONTRIBUTING.md). We need help with: cloud backends, hardware testing, documentation, examples.

---

## Summary

**You've learned:**
- ✅ How to install and build QRES
- ✅ How to compress/decompress data (Rust & Python)
- ✅ How to run the 100-node IoT demo
- ✅ How Byzantine tolerance works (attack injection)
- ✅ What the performance metrics mean

**Next actions:**
1. Run examples (10 minutes)
2. Read architecture docs (20 minutes)
3. Try custom predictor (30 minutes)
4. Deploy to hardware (1-2 hours)

**Questions?** 
- Open an issue: [GitHub Issues](https://github.com/CavinKrenik/QRES_RaaS/issues)
- Read docs: [docs/INDEX.md](../INDEX.md)
- Check paper: [DOI 10.5281/zenodo.18474976](https://doi.org/10.5281/zenodo.18474976)

---

**Welcome to QRES!** 🚀
