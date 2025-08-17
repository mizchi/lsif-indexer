# LSIF Indexer Benchmarks

## Overview

This project includes comprehensive benchmarks to measure the performance of various components:

- **Graph Construction**: Building code graphs of different sizes
- **Symbol Operations**: Finding symbols and references
- **LSIF Operations**: Generating and parsing LSIF format
- **Call Hierarchy**: Analyzing function call relationships
- **Edge Operations**: Adding edges with different patterns

## Running Benchmarks

### Quick Run
```bash
cargo bench
```

### Run Specific Benchmark Group
```bash
cargo bench graph_construction
cargo bench symbol_operations
cargo bench lsif_operations
cargo bench call_hierarchy
cargo bench edge_operations
```

### Generate HTML Report
```bash
cargo bench -- --save-baseline my_baseline
```

Results will be saved in `target/criterion/` directory.

## Benchmark Categories

### 1. Graph Construction
Tests the performance of building graphs with different sizes:
- Small graph: 10 symbols
- Medium graph: 100 symbols with complex edges
- Large graph: 1000 symbols with web of references

### 2. Symbol Operations
Measures the performance of:
- Finding symbols by ID
- Finding all references to a symbol
- Different graph sizes (small/medium/large)

### 3. LSIF Operations
Benchmarks for:
- Generating LSIF format from graphs
- Parsing LSIF back into graphs
- Different data sizes

### 4. Call Hierarchy
Tests the performance of:
- Finding outgoing calls (what a function calls)
- Finding incoming calls (who calls a function)
- Finding all paths between two functions
- Different depth limits

### 5. Edge Operations
Measures:
- Sequential edge addition
- Complex edge patterns
- Performance impact of different edge types

## Performance Tips

1. **Use Release Mode**: Always run benchmarks in release mode for accurate results
2. **Stable Environment**: Close other applications for consistent measurements
3. **Multiple Runs**: Use `--sample-size` to increase measurement accuracy

```bash
cargo bench -- --sample-size 200
```

## Comparing Performance

### Save Baseline
```bash
cargo bench -- --save-baseline before_changes
```

### Compare After Changes
```bash
cargo bench -- --baseline before_changes
```

## Profiling

For detailed profiling, use:

```bash
# CPU profiling with perf (Linux)
cargo bench --bench index_benchmark -- --profile-time=10

# Memory profiling with Valgrind
valgrind --tool=massif target/release/deps/index_benchmark-*
```

## Test Data

Benchmark test data is located in:
- `benches/fixtures/large_code.rs`: Sample Rust code for realistic testing

## Results Interpretation

The benchmarks measure:
- **Time**: How long operations take (ns/iter)
- **Throughput**: Operations per second
- **Memory**: Peak memory usage (when profiling)

Lower time values indicate better performance.

## CI Integration

Add to your CI pipeline:

```yaml
- name: Run benchmarks
  run: cargo bench -- --output-format bencher | tee output.txt

- name: Store benchmark result
  uses: benchmark-action/github-action-benchmark@v1
  with:
    tool: 'cargo'
    output-file-path: output.txt
```