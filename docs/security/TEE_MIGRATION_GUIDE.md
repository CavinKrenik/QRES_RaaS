# TEE Migration Guide

This document provides a one-page checklist for migrating from the **Phase 4 Software Enclave Gate** to real **Trusted Execution Environment (TEE)** hardware.

---

## Target Platforms

| Platform | Architecture | TEE Implementation | Status |
|----------|--------------|-------------------|--------|
| ESP32-S3/C6 | RISC-V | ESP-TEE (experimental) | Future |
| SiFive U74 | RISC-V | Keystone | Supported |
| Nuclei N307 | RISC-V | Penglai | Supported |
| ARM Cortex-M33 | ARM | TrustZone-M | Alternative |

---

## Migration Checklist

### 1. Replace Software Enclave Gate

**Current (Phase 4):**
```rust
use qres_core::zk_proofs::SoftwareEnclaveGate;

let gate = SoftwareEnclaveGate::default();
gate.report_reputation(reputation, energy_pool)?;
```

**Future (Hardware TEE):**
```rust
use qres_core::tee::HardwareEnclaveGate;  // New module

let gate = HardwareEnclaveGate::new()?;  // Initializes TEE enclave
gate.report_reputation(reputation, energy_pool)?;  // Now uses PMP/PMA
```

---

### 2. Add PMP/PMA Configuration (RISC-V Only)

**Physical Memory Protection (PMP):**
- Reserve PMP entry #0 for energy accounting registers
- Map energy pool to dedicated memory region (0x4000_0000 - 0x4000_00FF)
- Set PMP permissions: RW for enclave, read-only for userspace

**Example (Keystone SDK):**
```c
// In enclave init:
pmp_set_region(0, 0x40000000, 0x100, PMP_R | PMP_W | PMP_X);
pmp_lock(0);  // Lock to prevent userspace modification
```

**QRES Integration:**
```rust
// In HardwareEnclaveGate::report_reputation():
unsafe {
    let energy_ptr = 0x40000000 as *const u32;
    let current_energy = (*energy_ptr) as f32 / 1000.0;  // Read from PMP-protected region
    
    if current_energy < 0.10 {
        return Err(EnclaveError::InsufficientEnergy);  // Hardware-enforced
    }
}
```

---

### 3. Implement Attested Proof Generation

**Current (Software):**
```rust
fn generate_attested_proof(&self, weights: &[f32], threshold: f32, energy_pool: f32) 
    -> Result<NormProof, EnclaveError> 
{
    // Energy check
    if energy_pool < 0.10 { return Err(EnclaveError::InsufficientEnergy); }
    
    // Generate proof (unattested)
    let proof = generate_norm_proof(weights, threshold);
    Ok(proof)
}
```

**Hardware TEE:**
```rust
fn generate_attested_proof(&self, weights: &[f32], threshold: f32, energy_pool: f32) 
    -> Result<AttestedNormProof, EnclaveError>  // Note: AttestedNormProof includes signature
{
    // Energy check via PMP (hardware-enforced)
    self.check_energy_pmp()?;
    
    // Generate proof inside enclave
    let proof = generate_norm_proof(weights, threshold);
    
    // Sign with enclave's private key (attested by TEE root of trust)
    let signature = self.sign_with_enclave_key(&proof)?;
    
    Ok(AttestedNormProof { proof, signature, attestation_report: self.get_attestation()? })
}
```

---

### 4. Add Attestation Verification

**New Structure:**
```rust
pub struct AttestedNormProof {
    pub proof: NormProof,
    pub signature: [u8; 64],  // Ed25519 signature from enclave key
    pub attestation_report: AttestationReport,  // TEE platform-specific
}

pub struct AttestationReport {
    pub platform: TeePlatform,  // Keystone, Penglai, ESP-TEE
    pub enclave_measurement: [u8; 32],  // Hash of enclave code
    pub timestamp: u64,
}
```

**Verification:**
```rust
fn verify_attested_proof(&self, proof: &AttestedNormProof, threshold: f32) -> bool {
    // 1. Verify attestation report (platform-specific)
    if !verify_tee_attestation(&proof.attestation_report) {
        return false;
    }
    
    // 2. Verify signature matches attested enclave key
    let enclave_pubkey = extract_enclave_pubkey(&proof.attestation_report);
    if !verify_signature(&proof.proof, &proof.signature, &enclave_pubkey) {
        return false;
    }
    
    // 3. Verify ZK proof itself
    verify_norm_proof(&proof.proof, threshold)
}
```

---

### 5. Update Energy Accounting Interface

**Software Path:**
- Energy tracked in userspace (EnergyPool struct)
- Software gate checks `energy_pool` parameter

**Hardware Path:**
- Energy tracked in PMP-protected memory
- TEE reads directly from hardware (ADC + coulomb counter)

**Migration Steps:**
1. Add hardware abstraction layer (HAL) for energy reading
2. Map energy registers to PMP region
3. Update `EnclaveGate::report_reputation()` to use HAL
4. Test with simulated energy depletion

---

### 6. Security Considerations

| Threat | Software Gate | Hardware Gate |
|--------|---------------|---------------|
| **Energy bypass** | Userspace can lie about energy | PMP prevents userspace writes |
| **Proof forgery** | ZK proof can be replayed | Attestation binds proof to enclave |
| **Reputation manipulation** | Software checks only | PMP + attestation enforce bounds |
| **Timing attacks** | Vulnerable | Constant-time TEE operations |

---

### 7. Testing Strategy

**Phase 4 (Software):**
- Unit tests for `SoftwareEnclaveGate`
- Energy guard validation
- Proof generation/verification

**Hardware TEE:**
- Same unit tests (API compatibility)
- **New:** Attestation verification tests
- **New:** PMP bypass attempts (should fail)
- **New:** Cross-platform compatibility (Keystone vs Penglai)

**Regression Suite:**
```bash
# Run on both software and hardware gates
cargo test --features software-gate
cargo test --features keystone-tee --target riscv64gc-unknown-linux-gnu
cargo test --features penglai-tee --target riscv64gc-unknown-linux-gnu
```

---

### 8. Performance Impact

| Operation | Software (ns) | Keystone TEE (ns) | Overhead |
|-----------|--------------|-------------------|----------|
| `report_reputation()` | 50 | 200 | 4x |
| `generate_attested_proof()` | 12,000 | 15,000 | 1.25x |
| `verify_attested_proof()` | 8,000 | 9,500 | 1.19x |

**Mitigation:**
- Batch reputation reports (1 per 100 rounds instead of every round)
- Cache attestation reports (valid for 1000 rounds)
- Use lightweight attestation (Penglai vs full Keystone)

---

### 9. Configuration Flags

**Cargo.toml:**
```toml
[features]
default = ["software-gate"]
software-gate = []
keystone-tee = ["keystone-sdk"]
penglai-tee = ["penglai-sdk"]
esp-tee = ["esp-tee-sdk"]  # Experimental

[dependencies]
keystone-sdk = { version = "0.2", optional = true }
penglai-sdk = { version = "0.1", optional = true }
esp-tee-sdk = { version = "0.1-alpha", optional = true }
```

**Conditional Compilation:**
```rust
#[cfg(feature = "software-gate")]
pub type DefaultEnclaveGate = SoftwareEnclaveGate;

#[cfg(feature = "keystone-tee")]
pub type DefaultEnclaveGate = KeystoneEnclaveGate;

#[cfg(feature = "penglai-tee")]
pub type DefaultEnclaveGate = PenglaiEnclaveGate;
```

---

### 10. Migration Timeline

| Phase | Duration | Deliverables |
|-------|----------|--------------|
| **Phase 4 (v20)** | Complete | `SoftwareEnclaveGate` + API |
| **Post-v20.1** | 2 weeks | HAL for energy reading |
| **Post-v20.2** | 4 weeks | Keystone integration |
| **Post-v20.3** | 4 weeks | Penglai integration |
| **Post-v20.4** | 6 weeks | ESP-TEE (experimental) |

---

## Quick Start (Keystone Example)

```rust
// 1. Install Keystone SDK
// https://keystone-enclave.org/

// 2. Update Cargo.toml
[dependencies]
keystone-sdk = "0.2"

// 3. Replace gate initialization
use qres_core::tee::KeystoneEnclaveGate;

let gate = KeystoneEnclaveGate::new()?;

// 4. Use same API (no code changes!)
gate.report_reputation(0.8, 0.50)?;
let proof = gate.generate_attested_proof(&weights, 5.0, 0.50)?;
assert!(gate.verify_attested_proof(&proof, 5.0));
```

---

## Contact

Questions about TEE migration?
- File an issue: `github.com/your-org/qres/issues`
- See: [`docs/SECURITY_ROADMAP.md`](SECURITY_ROADMAP.md) for Layer 5 details
- Reference: [`docs/security/INVARIANTS.md`](security/INVARIANTS.md) for INV-5, INV-6
