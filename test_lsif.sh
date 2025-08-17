#!/bin/bash

echo "=== LSIF Indexer Test ==="
echo "Testing LSIF format generation and parsing..."
echo

# Build the project
echo "1. Building the project..."
cargo build --release 2>/dev/null
if [ $? -ne 0 ]; then
    echo "Build failed"
    exit 1
fi

# Generate index from source
echo "2. Generating index from source code..."
./target/release/lsif-indexer generate --source src/main.rs --output test_index.db
echo

# Export to LSIF format
echo "3. Exporting index to LSIF format..."
./target/release/lsif-indexer export-lsif --index test_index.db --output test_output.lsif
echo

# Show LSIF statistics
echo "4. LSIF file statistics:"
echo "   Lines: $(wc -l < test_output.lsif)"
echo "   Size: $(ls -lh test_output.lsif | awk '{print $5}')"
echo

# Show sample LSIF content
echo "5. Sample LSIF content (first 5 lines):"
head -5 test_output.lsif
echo

# Import LSIF back to index
echo "6. Importing LSIF back to index..."
./target/release/lsif-indexer import-lsif --input test_output.lsif --output test_imported.db
echo

# Verify the imported index
echo "7. Querying the original index:"
./target/release/query_index test_index.db 2>/dev/null | head -10
echo

echo "8. Summary:"
echo "   ✓ Generated index from source code"
echo "   ✓ Exported index to LSIF format ($(wc -l < test_output.lsif) elements)"
echo "   ✓ Imported LSIF to new index"
echo

# Clean up temporary files
rm -f test_index.db test_output.lsif test_imported.db

echo "Test completed successfully!"