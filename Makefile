.PHONY: all build test clean self-index help

# Default target
all: build

# Build the project
build:
	@echo "🔨 Building project..."
	@cargo build --release

# Run all tests
test:
	@echo "🧪 Running tests..."
	@cargo test

# Run specific test suites
test-unit:
	@echo "🧪 Running unit tests..."
	@cargo test --lib

test-integration:
	@echo "🧪 Running integration tests..."
	@cargo test --test '*'

test-reference:
	@echo "🧪 Running reference analysis tests..."
	@cargo test --test reference_analysis_test

# Clean temporary files and build artifacts
clean:
	@echo "🧹 Cleaning project..."
	@./scripts/clean.sh
	@cargo clean

# Clean only temporary files (keep build)
clean-temp:
	@echo "🧹 Cleaning temporary files..."
	@./scripts/clean.sh

# Self-index the codebase
self-index: build
	@./scripts/self-index.sh

# Run the interactive mode with self-index
interactive: self-index
	@echo "🚀 Starting interactive mode..."
	@./target/release/lsif interactive --db tmp/self-index.lsif

# Check code quality
check:
	@echo "🔍 Checking code..."
	@cargo check
	@cargo clippy -- -D warnings
	@cargo fmt -- --check

# Format code
fmt:
	@echo "✨ Formatting code..."
	@cargo fmt

# Run benchmarks
bench:
	@echo "⚡ Running benchmarks..."
	@cargo bench

# Show help
help:
	@echo "LSIF Indexer - Makefile targets"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Available targets:"
	@echo "  all          - Build the project (default)"
	@echo "  build        - Build the project in release mode"
	@echo "  test         - Run all tests"
	@echo "  test-unit    - Run unit tests only"
	@echo "  test-integration - Run integration tests only"
	@echo "  test-reference - Run reference analysis tests"
	@echo "  clean        - Clean all temporary files and build artifacts"
	@echo "  clean-temp   - Clean only temporary files (keep build)"
	@echo "  self-index   - Index the LSIF Indexer codebase itself"
	@echo "  interactive  - Run interactive mode with self-indexed data"
	@echo "  check        - Check code quality (check, clippy, format check)"
	@echo "  fmt          - Format code"
	@echo "  bench        - Run benchmarks"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make build       # Build the project"
	@echo "  make test        # Run all tests"
	@echo "  make self-index  # Index the codebase"
	@echo "  make clean       # Clean everything"