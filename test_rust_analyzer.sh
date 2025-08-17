#!/bin/bash

# rust-analyzerがインストールされているか確認
if ! command -v rust-analyzer &> /dev/null; then
    echo "rust-analyzer is not installed. Installing..."
    curl -L https://github.com/rust-analyzer/rust-analyzer/releases/latest/download/rust-analyzer-x86_64-unknown-linux-gnu.gz | gunzip -c - > ~/.local/bin/rust-analyzer
    chmod +x ~/.local/bin/rust-analyzer
fi

echo "Building test_lsp binary..."
cargo build --bin test_lsp

if [ $? -ne 0 ]; then
    echo "Build failed"
    exit 1
fi

echo "Running LSP test against src/main.rs..."
./target/debug/test_lsp src/main.rs

if [ $? -eq 0 ]; then
    echo "Test completed successfully!"
    echo "Results saved in lsp_symbols.json"
    
    # Show first few lines of result
    echo ""
    echo "Preview of results:"
    head -20 lsp_symbols.json
else
    echo "Test failed"
    exit 1
fi