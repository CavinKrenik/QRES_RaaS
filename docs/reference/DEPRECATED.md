# Deprecated Terminology & Migration Guide

**Document Version:** 1.0  
**Date:** February 5, 2026  
**QRES Version:** v21.0.0

---

## Overview

This document tracks deprecated terminology and legacy code in the QRES codebase, providing migration paths for users upgrading from older versions.

---

## Deprecated Terms

### 1. `GeneStorage` → `ModelPersistence`

**Status:** Deprecated in v20.0, removed in v21.0  
**Replacement:** `ModelPersistence`

**Reason:** "GeneStorage" was biological metaphor from early research prototype. "ModelPersistence" is clearer industry-standard terminology.

**Migration:**

```python
# Old (v19.x and earlier)
from qres.storage import GeneStorage
storage = GeneStorage(path="./checkpoints")
storage.save_gene(gene_data, metadata)

# New (v21.0+)
from qres import ModelPersistence
persist = ModelPersistence(storage_path="./checkpoints")
persist.save(compressed_data, metadata)
```

**Breaking Changes:**
- Class name changed
- Method `save_gene()` → `save()`
- Method `load_gene()` → `load()`
- Parameter `path` → `storage_path`

---

### 2. `SignedEpiphany` → `SignedModelUpdate`

**Status:** Deprecated in v20.0, removed in v21.0  
**Replacement:** `SignedModelUpdate`

**Reason:** "Epiphany" was metaphor for gradient updates. "ModelUpdate" is standard ML terminology.

**Migration:**

```python
# Old (v19.x and earlier)
from qres.swarm import SignedEpiphany
update = SignedEpiphany(
    epiphany=gradient_data,
    signature=sign(gradient_data, private_key),
    sender_id=peer_id
)

# New (v21.0+)
from qres.swarm import SignedModelUpdate
update = SignedModelUpdate(
    model_update=gradient_data,
    signature=sign(gradient_data, private_key),
    sender_id=peer_id
)
```

**Breaking Changes:**
- Class name changed
- Field `epiphany` → `model_update`
- Signature format unchanged (backward compatible)

---

### 3. `ViralBroadcast` → `GossipProtocol`

**Status:** Maintained for compatibility  
**Replacement:** Use `GossipProtocol` in new code

**Reason:** "ViralBroadcast" is accurate but "GossipProtocol" aligns with libp2p terminology.

**Migration:**

```python
# Old (still works in v21.0)
from qres.swarm import ViralBroadcast
protocol = ViralBroadcast(infection_rate=0.7)

# New (recommended in v21.0+)
from qres.swarm import GossipProtocol
protocol = GossipProtocol(fanout=7)  # fanout replaces infection_rate
```

**Status:** `ViralBroadcast` is aliased to `GossipProtocol` in v21.0 for backward compatibility. Will be removed in v22.0.

---

### 4. `CalmnessDetector` → `RegimeDetector`

**Status:** Deprecated in v18.0, removed in v21.0  
**Replacement:** `RegimeDetector`

**Reason:** "Calmness" was too narrow. "Regime" covers multiple operational modes (Calm, Storm, future extensions).

**Migration:**

```python
# Old (v17.x and earlier)
from qres.regime import CalmnessDetector
detector = CalmnessDetector(threshold=0.5)
is_calm = detector.is_calm()

# New (v21.0+)
from qres import RegimeDetector
detector = RegimeDetector(calm_threshold=0.5, storm_threshold=1.5)
regime = detector.current_regime()  # Returns "Calm" or "Storm"
```

**Breaking Changes:**
- Class name changed
- Method `is_calm()` → `current_regime()` (returns string, not boolean)
- Single threshold → separate `calm_threshold` and `storm_threshold`

---

## Legacy Code

### 1. `tools/swarm_sim/`

**Status:** Legacy simulation framework (pre-v15.0)  
**Location:** `tools/swarm_sim/`

**Description:**
Early Python simulation of swarm consensus before Rust implementation. Used for proof-of-concept research.

**Current Status:**
- **Maintained:** No (frozen since v15.0)
- **Tested:** No
- **Documented:** Minimal

**Should I use it?**
**No.** Use `qres_sim` crate or `examples/virtual_iot_network/` instead.

**Comparison:**

| Feature | `tools/swarm_sim/` (Legacy) | `qres_sim` (Current) |
|---------|----------------------------|---------------------|
| Language | Python | Rust |
| Performance | ~10 nodes | 1000+ nodes |
| Accuracy | Simplified model | Production-grade |
| TAAF Support | No | Yes |
| Byzantine Defense | Basic | Full (Grubbs' test) |
| Active Development | No | Yes |

**Migration Path:**

```bash
# Old: Run legacy simulator
python tools/swarm_sim/simulate.py --nodes 10

# New: Use virtual IoT network demo
cd examples/virtual_iot_network
cargo run --release -- \
  --nodes 100 \
  --byzantine-count 5 \
  --duration 60s
```

**Rationale for keeping:**
- Historical reference for academic papers (ICCV 2023, CVPR 2024 reproducibility)
- Some ablation studies reference legacy code
- Will move to `docs/archive/legacy/` in v22.0

---

### 2. `crates/benchmarks/old_bench_*.rs`

**Status:** Pre-v18.0 benchmark harness (before criterion migration)  
**Location:** `crates/benchmarks/src/old_bench_*.rs`

**Description:**
Custom benchmark harness before migration to criterion.rs framework.

**Current Status:**
- **Maintained:** No
- **Runs:** No (compile errors with modern Rust)
- **Purpose:** Historical comparison only

**Should I use it?**
**No.** Use `cargo bench` (criterion-based benchmarks).

**Migration:**

```bash
# Old: Run legacy benchmarks
cargo run -p benchmarks --bin old_bench_compression

# New: Use criterion benchmarks
cargo bench -p benchmarks compression
```

---

### 3. `data/iot/legacy_format/`

**Status:** Pre-v16.0 data format (before msgpack migration)

**Description:**
IoT dataset in custom binary format (before standardization on msgpack).

**Current Status:**
- **Reader:** Included in `tools/convert_legacy_data.py`
- **Writer:** Removed in v16.0
- **Documentation:** See `data/iot/legacy_format/README.md`

**Migration:**

```bash
# Convert legacy data to msgpack
python tools/convert_legacy_data.py \
  --input data/iot/legacy_format/ \
  --output data/iot/msgpack/
```

---

## Version Compatibility Matrix

| Feature | v17.x | v18.x | v19.x | v20.x | v21.x |
|---------|-------|-------|-------|-------|-------|
| `GeneStorage` | ✓ | ✓ | ⚠️ | ⚠️ | ❌ |
| `ModelPersistence` | ❌ | ❌ | ✓ | ✓ | ✓ |
| `SignedEpiphany` | ✓ | ✓ | ⚠️ | ⚠️ | ❌ |
| `SignedModelUpdate` | ❌ | ❌ | ✓ | ✓ | ✓ |
| `CalmnessDetector` | ⚠️ | ❌ | ❌ | ❌ | ❌ |
| `RegimeDetector` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `ViralBroadcast` | ✓ | ✓ | ✓ | ✓ | ⚠️ |
| `GossipProtocol` | ❌ | ❌ | ❌ | ✓ | ✓ |
| Legacy `swarm_sim` | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ⚠️ |
| `qres_sim` crate | ✓ | ✓ | ✓ | ✓ | ✓ |

**Legend:**
- ✓ Supported
- ⚠️ Deprecated (works with warnings)
- ❌ Removed

---

## Automated Migration Tool

**Coming in v21.1:** `qres-migrate` CLI tool

```bash
# Scan codebase for deprecated APIs
qres-migrate scan --path ./my_project/

# Apply automated fixes
qres-migrate fix --path ./my_project/ --from v19 --to v21

# Generate migration report
qres-migrate report --format markdown > MIGRATION.md
```

**Status:** Prototype in `tools/migrate/` (not yet stable)

---

## Deprecation Policy

QRES follows semantic versioning with this deprecation policy:

1. **Minor version (e.g., v20.5):** Deprecation warnings added, old API still works
2. **Next minor (e.g., v20.6):** Documentation updated, migration guide published
3. **Next major (e.g., v21.0):** Old API removed, breaking changes allowed

**Example Timeline:**
- v20.0 (June 2025): `GeneStorage` deprecated with warnings
- v20.5 (Aug 2025): Migration guide published
- v21.0 (Jan 2026): `GeneStorage` removed

---

## Getting Help

**Found deprecated code not listed here?**
1. Search [GitHub Issues](https://github.com/your-org/qres/issues) for migration guides
2. Check [Discussions](https://github.com/your-org/qres/discussions) for Q&A
3. Open new issue with `migration` label

**Automated detection:**
```bash
# Run clippy with migration lints
cargo clippy -- -W deprecated

# Python deprecation warnings
python -W default::DeprecationWarning your_script.py
```

---

## References

- [CHANGELOG.md](../../CHANGELOG.md) - Full version history
- [Migration Guide (v20→v21)](../guides/MIGRATION_v20_to_v21.md)
- [API Reference](../reference/API_REFERENCE.md) - Current stable APIs
- [Roadmap](../roadmap/) - Future deprecation plans

---

**Last updated:** February 5, 2026  
**Maintainer:** QRES Core Team  
**Review Schedule:** Quarterly (next review: May 2026)
