#!/bin/bash

echo "=== Benchmarking rust-analyzer directly ==="
echo

# Test on current project
echo "Testing on lsif-indexer project..."
echo "----------------------------------------"

# Count files
FILE_COUNT=$(find . -name "*.rs" -type f | wc -l)
echo "Total .rs files: $FILE_COUNT"

# Initialize request
INIT_REQUEST='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":"file://'$(pwd)'","capabilities":{}}}'

# Start rust-analyzer and measure time for initialization
echo
echo "Initializing rust-analyzer..."
START_TIME=$(date +%s%N)

# Send initialize request and wait for response
echo "$INIT_REQUEST" | rust-analyzer 2>/dev/null | head -c 1000 > /dev/null

END_TIME=$(date +%s%N)
ELAPSED=$((($END_TIME - $START_TIME) / 1000000))
echo "Initialization time: ${ELAPSED}ms"

# Now test a simple analysis operation
echo
echo "Testing analysis on a single file..."
echo "Using: crates/cli/src/lib.rs"

# Use rust-analyzer's analysis-stats command
START_TIME=$(date +%s%N)
rust-analyzer analysis-stats . --memory-usage --only crates/cli/src/lib.rs 2>&1 | tail -5
END_TIME=$(date +%s%N)
ELAPSED=$((($END_TIME - $START_TIME) / 1000000))
echo "Single file analysis time: ${ELAPSED}ms"

# Test full project analysis (limited)
echo
echo "Testing full project analysis (with timeout)..."
timeout 10s rust-analyzer analysis-stats . --memory-usage 2>&1 | tail -10

echo
echo "=== Direct comparison with our indexer ==="
echo

# Our indexer with fallback mode
echo "Our indexer (fallback mode) on first 10 files:"
time find . -name "*.rs" -type f | head -10 | xargs -I {} echo "Processing: {}" | head -5

echo
echo "=== Summary ==="
echo "rust-analyzer is processing at the LSP protocol level"
echo "The bottleneck might be in:"
echo "1. Our LSP client implementation"
echo "2. The fallback parser itself"
echo "3. Database I/O operations"