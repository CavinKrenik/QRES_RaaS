#!/bin/bash
# QRES Paper Experiment Runner
# Reproduces all experiments from the QRES paper (Sections IV-VII)
#
# Prerequisites:
#   - Python 3.10+ with numpy, scipy, matplotlib, pandas
#   - Rust toolchain (for cargo build/test)
#
# Usage:
#   cd <repo_root>
#   bash evaluation/reproducibility/scripts/run_paper_experiments.sh
#
# Output:
#   - evaluation/results/*.json          (raw experiment data)
#   - docs/RaaS_Paper/figures/*.pdf      (paper figures)
#   - docs/RaaS_Paper/tables/*.tex       (LaTeX tables)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
cd "$REPO_ROOT"

echo "============================================"
echo " QRES Paper Experiment Suite"
echo " Repository: $(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
echo " Date: $(date -u '+%Y-%m-%d %H:%M UTC')"
echo "============================================"
echo ""

# Step 1: Build and test Rust core
echo "[1/4] Building Rust core..."
cargo build --release --workspace 2>&1 | tail -5
echo ""

echo "[2/4] Running Rust tests..."
cargo test --workspace 2>&1 | tail -10
echo ""

# Step 2: Detect Python
PYTHON=""
if [ -f ".venv/bin/python" ]; then
    PYTHON=".venv/bin/python"
elif [ -f ".venv/Scripts/python.exe" ]; then
    PYTHON=".venv/Scripts/python.exe"
elif command -v python3 &>/dev/null; then
    PYTHON="python3"
elif command -v python &>/dev/null; then
    PYTHON="python"
else
    echo "ERROR: Python not found. Install Python 3.10+ and create a venv."
    exit 1
fi
echo "Using Python: $PYTHON"
$PYTHON --version
echo ""

# Step 3: Check Python deps
echo "[3/4] Checking Python dependencies..."
$PYTHON -c "import numpy, scipy, matplotlib, pandas; print('All dependencies available')" || {
    echo "Installing missing dependencies..."
    $PYTHON -m pip install numpy scipy matplotlib pandas
}
echo ""

# Step 4: Run paper experiments
echo "[4/4] Running paper experiments..."
echo "  This generates all figures, tables, and result files."
echo ""
$PYTHON evaluation/analysis/paper_experiments.py

echo ""
echo "============================================"
echo " Experiment suite complete!"
echo ""
echo " Generated artifacts:"
echo "   Figures: docs/RaaS_Paper/figures/*.pdf"
echo "   Tables:  docs/RaaS_Paper/tables/*.tex"
echo "   Data:    evaluation/results/*.json"
echo ""
echo " To rebuild the paper:"
echo "   cd docs/RaaS_Paper && pdflatex main && bibtex main && pdflatex main && pdflatex main"
echo "============================================"
