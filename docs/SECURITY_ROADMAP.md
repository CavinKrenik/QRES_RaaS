Here is the updated content for `docs/SECURITY_ROADMAP.md`, reflecting the new "Layered" security architecture and the successful implementation of the Ghost Protocol (Phase 3).

### File: `docs/SECURITY_ROADMAP.md`

```markdown
# QRES Security Architecture & Roadmap (2026)

This document outlines the **Defense-in-Depth** security architecture of the QRES distributed system. It tracks the implementation status of our "Immune System" layers, designed to protect the network from adversarial attacks while preserving user privacy.

> **Implementation Guide:** See [docs/guides/SECURITY_IMPLEMENTATION_GUIDE.md](guides/SECURITY_IMPLEMENTATION_GUIDE.md)

---

## Layer 1: Network & Identity (The Outer Shell)
**Focus:** Securing the P2P transport and ensuring node accountability.

### Implemented
- **Ed25519 Signatures**: Guarantees authenticity of all model updates (v13).
- **Node PKI**: Enforced identity verification via `libp2p` Noise protocol.
- **Replay Prevention**: Nonces and timestamps prevent replay attacks.

### Roadmap
- **Hardware Enclaves (TEE/SGX)**: Hardware-backed key protection.

---

## Layer 2: Trust & Reputation (The Gatekeeper)
**Focus:** Filtering malicious actors based on historical behavior and mathematical validity.

### Implemented
- **Reputation Scoring (v16.5)**: Persistent trust tracking.
    - **Reward**: `+0.01` for accepted updates.
    - **Punish**: `-0.1` for updates rejected by Krum.
    - **Ban**: Trust `< 0.2` triggers simple blocklist.
- **The Gatekeeper**: Logic that binds aggregation results back to the P2P identity layer.

### Roadmap
- **Federated Reputation**: Sharing reputation scores (Web of Trust) to accelerate ban propagation.

---

## Layer 3: Privacy & Zero-Knowledge (The Ghost Protocol)
**Focus:** Protecting the confidentiality of raw data and individual updates from peers and aggregators.

### Implemented (v16.5)
- [x] **Differential Privacy (DP)**: Gaussian noise addition to `I16F16` gradients to prevent reverse-engineering.
- [x] **Secure Aggregation**: Pairwise masking (x25519 + ChaCha20) ensures aggregators see only the global sum.
- [x] **Zero-Knowledge Proofs (ZK)**: Pedersen Commitments proving that masked updates are within valid bounds (Norm Proofs).
- [x] **Ghost Packet**: Encapsulated transport structure (`GhostUpdate`) carrying the masked payload and proofs.

### Roadmap
- **Full Range Proofs**: Proving individual weight elements are within bounds (Bulletproofs).
- **Homomorphic Encryption**: Fully encrypted computation (Long term).

---

## Layer 4: Algorithmic Robustness (The Immune System)
**Focus:** Mathematical resilience against Byzantine faults and poisoning.

### Implemented
- **Trimmed Mean (v19.0)**: Replaced Krum. Statistically robust aggregation that removes the top-$f$ and bottom-$f$ outliers per dimension.
- **Krum Algorithm (Legacy)**: Retained as fallback for small swarms ($N < 5$).
- **Dreaming Sanity Check**: Validates synthetic "dreamt" data against real validation buffers.

### Roadmap
- **Pre-Merge Validation**: Local validation set testing for *all* incoming updates (not just dreams).
- **Sybil Resistance (PoW)**: Lightweight proof-of-work for identity creation (if PKI is not used).

---

## Layer 5: Hardware-Attested Trust (v20+)
**Focus:** Trusted Execution Environments (TEE) for silicon-level security guarantees.

### Implemented (v20 Phase 4)
- [x] **EnclaveGate Trait**: Unified API for hardware-abstracted security operations
- [x] **Software Enclave Gate**: Mock implementation with PMP/PMA simulation
  - Energy guard: `report_reputation()` fails if `EnergyPool < 10%` (INV-5)
  - ZK proof generation with energy accounting
  - API-compatible with future TEE backends
- [x] **TEE Migration Guide**: One-page checklist for Keystone/Penglai/ESP-TEE integration

### Roadmap (Post-v20)
- **Keystone TEE Integration**: RISC-V Keystone SDK for SiFive U74
- **Penglai TEE Integration**: Nuclei N307 support
- **ESP-TEE Integration**: Experimental ESP32-S3/C6 support
- **Attestation Verification**: Platform-specific attestation report validation
- **Hardware Energy Accounting**: Direct PMP-protected register reads (no userspace bypass)
- **Attested ZK Proofs**: Proofs signed with enclave key (prevents forgery)

**Security Benefits:**
- **Physical Memory Protection (PMP)**: Prevents userspace from bypassing energy guards
- **Hardware-Attested Proofs**: ZK proofs cryptographically bound to enclave identity
- **Constant-Time Operations**: TEE-enforced timing attack resistance
- **Sealed Storage**: Lamarckian weight recovery with hardware encryption

**Migration Path:**
1. **Phase 4 (v20):** Software gate establishes API contract
2. **Post-v20.1:** Hardware Abstraction Layer (HAL) for energy reading
3. **Post-v20.2:** Keystone integration (primary RISC-V target)
4. **Post-v20.3:** Penglai integration (alternative RISC-V)
5. **Post-v20.4:** ESP-TEE (experimental embedded TEE)

See [`TEE_MIGRATION_GUIDE.md`](TEE_MIGRATION_GUIDE.md) for implementation details.

---

## Attack Mitigation Matrix

| Attack Vector | Primary Defense | Secondary Defense |
|:---|:---|:---|
| **Sybil Attack** | Node PKI (Layer 1) | Reputation Cost (Layer 2) |
| **Model Poisoning** | Trimmed Mean (Layer 4) | Reputation Banning (Layer 2) |
| **Gradient Inversion** | Differential Privacy (Layer 3) | Secure Aggregation (Layer 3) |
| **"Lazy Worker" Spoofing** | ZK Proofs (Layer 3) | Reputation (Layer 2) |
| **Man-in-the-Middle** | Ed25519 Signatures (Layer 1) | Transport Encryption (Layer 1) |
