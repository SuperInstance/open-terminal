#!/bin/bash
# Test just the math_analysis module files by compiling each as a standalone test
set -euo pipefail
cd /tmp/intelligent-terminal/tools/wta

# Run cargo test with --test-threads=1 and specific test filter
cargo test --features math-tools -- "math_analysis" --test-threads=1 --nocapture 2>&1
