# QRES Rust Examples

Low-level examples demonstrating `qres_core` usage in Rust.

## Examples

### 01_basic_compression

Minimal `no_std` compatible compression example.

**Features:**
- `qres_core` usage without `std`
- Q16.16 fixed-point arithmetic
- Deterministic compression
- Suitable for embedded targets (ESP32-C6, STM32, etc.)

**Run:**
```bash
cd examples/rust/01_basic_compression
cargo run --release
```

**Expected Output:**
```
QRES Basic Compression (no_std)
================================
Original:  48 bytes
Compressed: 12 bytes
Ratio: 4.00x
✓ Decompression verified
```

### 02_custom_predictor

Implement custom `Predictor` trait for domain-specific compression.

**Features:**
- Custom predictor implementation
- Linear regression example
- Integration with `qres_core::compress`
- Training and serialization

**Run:**
```bash
cd examples/rust/02_custom_predictor
cargo run --release
```

## Prerequisites

- Rust 1.75+ ([rustup.rs](https://rustup.rs/))
- For embedded targets:
  ```bash
  rustup target add thumbv7em-none-eabihf  # ARM Cortex-M4/M7
  rustup target add riscv32imc-unknown-none-elf  # RISC-V
  ```

## Building for Embedded

### ESP32-C6 (RISC-V)

```bash
cd 01_basic_compression
cargo build --release --target riscv32imc-unknown-none-elf --no-default-features
```

### STM32 (ARM Cortex-M4)

```bash
cargo build --release --target thumbv7em-none-eabihf --no-default-features
```

## API Documentation

Full Rust API: [docs/reference/API_REFERENCE.md](../../docs/reference/API_REFERENCE.md)

Core traits:
- `qres_core::Predictor` - Custom compression models
- `qres_core::Aggregator` - Byzantine-tolerant consensus
- `qres_core::ModelPersistence` - State serialization

## Performance

Benchmarks on x86_64 (i7-10700K, release build):

| Operation | Time | Throughput |
|-----------|------|------------|
| `compress()` | 100ns | 10M ops/sec |
| `decompress()` | 80ns | 12M ops/sec |
| `Predictor::forward()` | 50ns | 20M ops/sec |

**Memory:** <1 KB per-node overhead (fixed-size buffers, no heap)

## Cross-Compilation

Use `cross` for easy cross-compilation:

```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

Supported targets:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu` (ARM64/Raspberry Pi 4)
- `armv7-unknown-linux-gnueabihf` (ARM32/Raspberry Pi 3)
- `wasm32-unknown-unknown` (WebAssembly)
- `riscv32imc-unknown-none-elf` (ESP32-C6)
- `thumbv7em-none-eabihf` (STM32)

## Debugging

Enable debug logging:

```bash
RUST_LOG=debug cargo run --release
```

Use `gdb` for embedded debugging:
```bash
cargo embed --release --chip STM32F407VGTx
```

## Testing

Run unit tests:
```bash
cargo test --release
```

Run benchmarks:
```bash
cargo bench
```

## Next Steps

- **Python API:** See [examples/python/](../python/) for high-level usage
- **Full Demo:** See [examples/virtual_iot_network/](../virtual_iot_network/) for 100-node mesh
- **Hardware:** See [docs/deployment/](../../docs/deployment/) for ESP32-C6/Pi deployment guides

## License

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE).
