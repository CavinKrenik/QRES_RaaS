.PHONY: help test test-all bench verify fmt clippy doc coverage build build-release python clean

# Show available targets
help:
	@echo "Available targets:"
	@echo "  test           Run core library tests"
	@echo "  test-all       Run full workspace tests"
	@echo "  bench          Run benchmarks"
	@echo "  verify         Run verification tests"
	@echo "  fmt            Check formatting"
	@echo "  clippy         Run clippy lints"
	@echo "  doc            Generate API documentation"
	@echo "  coverage       Generate code coverage report"
	@echo "  build          Debug build"
	@echo "  build-release  Release build"
	@echo "  python         Build Python bindings"
	@echo "  clean          Remove build artifacts"

# Run core library tests (std features)
test:
	cargo test -p qres_core --features std --release

# Run full workspace tests (all features, serial execution for determinism)
test-all:
	cargo test --workspace --all-features --release -- --test-threads=1

# Run benchmarks
bench:
	cargo bench --workspace --all-features

# Run verification tests (invariants + determinism)
verify:
	cargo test -p qres_core --features std --release -- verify

# Check formatting
fmt:
	cargo fmt --all -- --check

# Run clippy lints (warnings are errors)
clippy:
	cargo clippy --workspace --all-features --all-targets -- -D warnings

# Generate API documentation (including private items)
doc:
	cargo doc --workspace --all-features --no-deps --document-private-items
	@echo "Open target/doc/qres_core/index.html in your browser"

# Generate code coverage report (requires cargo-tarpaulin)
coverage:
	cargo tarpaulin --workspace --all-features --out Html --output-dir coverage/ --exclude-files "crates/qres_wasm/*" "target/*"
	@echo "Open coverage/tarpaulin-report.html in your browser"

# Debug build (all workspace members)
build:
	cargo build --workspace

# Release build (all workspace members)
build-release:
	cargo build --workspace --release

# Build Python bindings (requires maturin: pip install maturin)
python:
	cd bindings/python && maturin develop --release

# Remove build artifacts
clean:
	cargo clean
	rm -rf coverage/
