#!/usr/bin/env python3
"""
Example 01: Basic Compression & Decompression
==============================================

Demonstrates the core QRES compression API with deterministic guarantees.

Features:
- Simple compress/decompress workflow
- Deterministic compression (same input → same output)
- Multiple usage hints for optimal predictor selection
- Compression ratio analysis

Requirements:
    pip install qres-raas  # Or: cd bindings/python && maturin develop --release

References:
    - API Reference: docs/reference/API_REFERENCE.md
    - Architecture: docs/reference/ARCHITECTURE.md
"""

from qres import QRES_API
import sys


def main():
    print("=" * 60)
    print("QRES v21.0 - Basic Compression Example")
    print("=" * 60)
    
    # Initialize API in hybrid mode (adaptive regime detection)
    api = QRES_API(mode="hybrid")
    print("✓ QRES API initialized (mode=hybrid)\n")
    
    # Example 1: Text compression
    print("Example 1: Text Data")
    print("-" * 60)
    text_data = b"The quick brown fox jumps over the lazy dog. " * 10
    
    compressed = api.compress(text_data, usage_hint="text")
    decompressed = api.decompress(compressed)
    
    assert text_data == decompressed, "Decompression mismatch!"
    
    ratio = len(text_data) / len(compressed)
    savings = len(text_data) - len(compressed)
    
    print(f"Original size:    {len(text_data):,} bytes")
    print(f"Compressed size:  {len(compressed):,} bytes")
    print(f"Compression ratio: {ratio:.2f}x")
    print(f"Bandwidth saved:  {savings:,} bytes ({savings/len(text_data)*100:.1f}%)")
    print("✓ Deterministic decompression verified\n")
    
    # Example 2: IoT sensor data
    print("Example 2: IoT Sensor Data")
    print("-" * 60)
    sensor_data = b"sensor_id=42,temp=23.5C,humidity=65%,pressure=1013hPa,timestamp=1709654400"
    
    compressed_iot = api.compress(sensor_data, usage_hint="iot")
    decompressed_iot = api.decompress(compressed_iot)
    
    assert sensor_data == decompressed_iot
    
    ratio_iot = len(sensor_data) / len(compressed_iot)
    print(f"Original size:    {len(sensor_data):,} bytes")
    print(f"Compressed size:  {len(compressed_iot):,} bytes")
    print(f"Compression ratio: {ratio_iot:.2f}x")
    print("✓ IoT-optimized compression verified\n")
    
    # Example 3: Determinism verification (same input → same output)
    print("Example 3: Determinism Verification")
    print("-" * 60)
    data = b"Deterministic fixed-point compression" * 5
    
    compress1 = api.compress(data, usage_hint="auto")
    compress2 = api.compress(data, usage_hint="auto")
    compress3 = api.compress(data, usage_hint="auto")
    
    if compress1 == compress2 == compress3:
        print("✓ Determinism verified: 3 consecutive compressions identical")
        print(f"  Hash: {hash(compress1)}")
    else:
        print("✗ Warning: Non-deterministic compression detected")
        sys.exit(1)
    
    print("\n" + "=" * 60)
    print("Summary: All compression tests passed!")
    print("=" * 60)
    print("\nNext Steps:")
    print("  - See 02_multimodal_taaf.py for TAAF fusion")
    print("  - See 03_swarm_node.py for P2P swarm participation")
    print("  - Read docs/reference/API_REFERENCE.md for full API")


if __name__ == "__main__":
    try:
        main()
    except ImportError as e:
        print(f"✗ Error: {e}")
        print("\nInstallation:")
        print("  From source: cd bindings/python && maturin develop --release")
        print("  From PyPI:   pip install qres-raas")
        sys.exit(1)
    except Exception as e:
        print(f"✗ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
