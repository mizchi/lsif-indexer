#!/bin/bash
# CI用のLSPテストスクリプト

set -e

echo "=== LSP Server Integration Tests ==="

# 環境変数の設定
export PATH="/usr/local/bin:$HOME/go/bin:$PATH"
export RUST_BACKTRACE=1
export RUST_LOG=debug

# LSPサーバーの存在確認
echo "Checking LSP servers..."

if command -v rust-analyzer &> /dev/null; then
    echo "✅ rust-analyzer: $(rust-analyzer --version)"
    RUST_ANALYZER_AVAILABLE=1
else
    echo "❌ rust-analyzer not found"
    RUST_ANALYZER_AVAILABLE=0
fi

if command -v gopls &> /dev/null; then
    echo "✅ gopls: $(gopls version 2>&1 | head -1)"
    GOPLS_AVAILABLE=1
else
    echo "❌ gopls not found"
    GOPLS_AVAILABLE=0
fi

if command -v tsgo &> /dev/null; then
    echo "✅ tsgo: $(tsgo --version)"
    TSGO_AVAILABLE=1
else
    echo "❌ tsgo not found"
    TSGO_AVAILABLE=0
fi

if command -v pyright &> /dev/null; then
    echo "✅ pyright: $(pyright --version)"
    PYRIGHT_AVAILABLE=1
else
    echo "❌ pyright not found"
    PYRIGHT_AVAILABLE=0
fi

echo ""
echo "=== Running LSP Integration Tests ==="

# rust-analyzerテスト
if [ $RUST_ANALYZER_AVAILABLE -eq 1 ]; then
    echo "Testing rust-analyzer integration..."
    cargo test --package lsp --lib test_with_real_rust_analyzer -- --include-ignored --nocapture || {
        echo "⚠️  rust-analyzer test failed (non-critical)"
    }
else
    echo "⚠️  Skipping rust-analyzer test (not installed)"
fi

# goplsテスト
if [ $GOPLS_AVAILABLE -eq 1 ]; then
    echo "Testing gopls integration..."
    cargo test --package lsp --lib test_with_real_gopls -- --include-ignored --nocapture || {
        echo "⚠️  gopls test failed (non-critical)"
    }
else
    echo "⚠️  Skipping gopls test (not installed)"
fi

# tsgoテスト（必須）
if [ $TSGO_AVAILABLE -eq 1 ]; then
    echo "Testing tsgo integration..."
    cargo test --package lsp --lib test_with_real_tsgo -- --nocapture || {
        echo "❌ tsgo test failed!"
        exit 1
    }
    echo "✅ tsgo test passed"
else
    echo "❌ tsgo not found - this is required!"
    exit 1
fi

echo ""
echo "=== Multi-Language Indexing Test ==="

# テスト用プロジェクトの作成
mkdir -p test-projects

# Rustプロジェクト
if [ $RUST_ANALYZER_AVAILABLE -eq 1 ]; then
    mkdir -p test-projects/rust
    cat > test-projects/rust/main.rs << 'EOF'
fn main() {
    println!("Hello, Rust!");
}

struct User {
    name: String,
    age: u32,
}

impl User {
    fn new(name: String, age: u32) -> Self {
        User { name, age }
    }
}
EOF
    
    echo "Indexing Rust project..."
    ./target/release/lsif index-project -p test-projects/rust -o tmp/rust.db -l rust || true
fi

# Goプロジェクト
if [ $GOPLS_AVAILABLE -eq 1 ]; then
    mkdir -p test-projects/go
    cat > test-projects/go/main.go << 'EOF'
package main

import "fmt"

type User struct {
    Name string
    Age  int
}

func main() {
    fmt.Println("Hello, Go!")
}
EOF
    
    echo "Indexing Go project..."
    ./target/release/lsif index-project -p test-projects/go -o tmp/go.db -l go || true
fi

# TypeScriptプロジェクト
if [ $TSGO_AVAILABLE -eq 1 ]; then
    mkdir -p test-projects/typescript
    cat > test-projects/typescript/index.ts << 'EOF'
interface User {
    name: string;
    age: number;
}

class UserService {
    private users: User[] = [];
    
    addUser(user: User): void {
        this.users.push(user);
    }
}

export { User, UserService };
EOF
    
    cat > test-projects/typescript/tsconfig.json << 'EOF'
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "strict": true
  }
}
EOF
    
    echo "Indexing TypeScript project..."
    ./target/release/lsif index-project -p test-projects/typescript -o tmp/ts.db -l typescript
    
    # TypeScriptのインデックスが作成されたか確認
    if [ -f tmp/ts.db ]; then
        echo "✅ TypeScript indexing successful"
        ./target/release/lsif query -i tmp/ts.db --query-type symbols | head -10
    else
        echo "❌ TypeScript indexing failed!"
        exit 1
    fi
fi

echo ""
echo "=== All LSP Integration Tests Completed ==="