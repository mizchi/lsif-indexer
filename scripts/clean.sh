#!/bin/bash

# LSIF Indexer Cleanup Script
# This script removes temporary files and test artifacts

set -e

echo "🧹 Cleaning up temporary files..."

# Remove test index files
if ls test-* 1> /dev/null 2>&1; then
    echo "  Removing test-* files..."
    rm -rf test-*
fi

# Remove self-index files
if ls self-index* 1> /dev/null 2>&1; then
    echo "  Removing self-index* files..."
    rm -rf self-index*
fi

# Clean tmp directory but keep the directory itself
if [ -d "tmp" ]; then
    echo "  Cleaning tmp/ directory..."
    rm -rf tmp/*
    # Ensure tmp directory exists for future use
    mkdir -p tmp
fi

# Remove any .db files and directories in root (except in target/)
for item in *.db; do
    if [ -e "$item" ]; then
        echo "  Removing $item..."
        rm -rf "$item"
    fi
done

# Remove any .index files in root
if ls *.index 1> /dev/null 2>&1; then
    echo "  Removing *.index files..."
    rm -f *.index
fi

# Remove any sled database directories (look for directories with sled pattern)
for dir in *sled*; do
    if [ -d "$dir" ]; then
        echo "  Removing $dir directory..."
        rm -rf "$dir"
    fi
done

echo "✨ Cleanup complete!"

# Optional: Show remaining files
echo ""
echo "📁 Current directory status:"
ls -la | grep -vE "(^\.|target|src|tests|benches|Cargo|README|LICENSE)" | head -20 || echo "  No temporary files found"