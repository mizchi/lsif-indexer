#!/bin/bash

echo "=== Call Hierarchy Analysis Test ==="
echo

# Build the project
echo "1. Building the project..."
cargo build --release 2>/dev/null
if [ $? -ne 0 ]; then
    echo "Build failed"
    exit 1
fi

# Generate index for sample code
echo "2. Generating index for sample code..."
./target/release/lsif-indexer generate --source tests/fixtures/sample.rs --output sample_test.db

# Test call hierarchy commands
echo
echo "3. Testing call hierarchy commands..."
echo

echo "=== Outgoing calls from main ==="
./target/release/lsif-indexer call-hierarchy \
    --index sample_test.db \
    --symbol "tests/fixtures/sample.rs#2:main" \
    --direction outgoing \
    --max-depth 3

echo
echo "=== Run unit tests ==="
cargo test --lib call_hierarchy -- --nocapture

echo
echo "=== Run integration tests ==="
cargo test --test call_hierarchy_test -- --nocapture

echo
echo "Test completed!"

# Clean up
rm -f sample_test.db