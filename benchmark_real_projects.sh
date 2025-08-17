#!/bin/bash
set -e

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  LSIF Indexer Real Project Benchmark  ${NC}"
echo -e "${GREEN}========================================${NC}"

# ビルド
echo -e "\n${YELLOW}Building LSIF Indexer...${NC}"
cargo build --release

# 結果ディレクトリ作成
RESULTS_DIR="benchmark_results_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

# プロジェクト情報を収集
echo -e "\n${YELLOW}Collecting project statistics...${NC}"

# React (TypeScript/JavaScript)
echo -e "\n${GREEN}React Project Stats:${NC}"
REACT_JS_FILES=$(find /tmp/benchmark_repos/react -name "*.js" -o -name "*.jsx" 2>/dev/null | wc -l || echo 0)
REACT_TS_FILES=$(find /tmp/benchmark_repos/react -name "*.ts" -o -name "*.tsx" 2>/dev/null | wc -l || echo 0)
REACT_TOTAL_LOC=$(find /tmp/benchmark_repos/react \( -name "*.js" -o -name "*.jsx" -o -name "*.ts" -o -name "*.tsx" \) -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo 0)
echo "  JavaScript/JSX files: $REACT_JS_FILES"
echo "  TypeScript/TSX files: $REACT_TS_FILES"
echo "  Total lines of code: $REACT_TOTAL_LOC"

# Deno (Rust)
echo -e "\n${GREEN}Deno Project Stats:${NC}"
DENO_RS_FILES=$(find /tmp/benchmark_repos/deno -name "*.rs" 2>/dev/null | wc -l || echo 0)
DENO_TOTAL_LOC=$(find /tmp/benchmark_repos/deno -name "*.rs" -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}' || echo 0)
echo "  Rust files: $DENO_RS_FILES"
echo "  Total lines of code: $DENO_TOTAL_LOC"

# ベンチマーク関数
run_benchmark() {
    local PROJECT_NAME=$1
    local PROJECT_PATH=$2
    local FILE_PATTERN=$3
    local LANGUAGE=$4
    
    echo -e "\n${GREEN}Benchmarking $PROJECT_NAME...${NC}"
    
    # インデックス作成のベンチマーク（サンプルファイルのみ）
    echo "Finding sample files..."
    SAMPLE_FILES=$(find "$PROJECT_PATH" -name "$FILE_PATTERN" 2>/dev/null | head -10)
    
    if [ -z "$SAMPLE_FILES" ]; then
        echo -e "${RED}No $FILE_PATTERN files found in $PROJECT_PATH${NC}"
        return
    fi
    
    TOTAL_TIME=0
    FILE_COUNT=0
    
    for FILE in $SAMPLE_FILES; do
        if [ -f "$FILE" ]; then
            echo "  Indexing: $(basename $FILE)"
            
            # 時間計測開始
            START_TIME=$(date +%s%N)
            
            # インデックス生成
            timeout 30 ./target/release/lsif-indexer generate \
                --source "$FILE" \
                --output "/tmp/index_${PROJECT_NAME}_$(basename $FILE).db" \
                --language "$LANGUAGE" 2>&1 | tail -5 || true
            
            # 時間計測終了
            END_TIME=$(date +%s%N)
            ELAPSED=$((($END_TIME - $START_TIME) / 1000000)) # ミリ秒に変換
            
            echo "    Time: ${ELAPSED}ms"
            TOTAL_TIME=$((TOTAL_TIME + ELAPSED))
            FILE_COUNT=$((FILE_COUNT + 1))
        fi
    done
    
    if [ $FILE_COUNT -gt 0 ]; then
        AVG_TIME=$((TOTAL_TIME / FILE_COUNT))
        echo -e "${YELLOW}Average indexing time: ${AVG_TIME}ms per file${NC}"
        
        # 結果をファイルに保存
        echo "$PROJECT_NAME,$FILE_COUNT,$TOTAL_TIME,$AVG_TIME" >> "$RESULTS_DIR/benchmark_results.csv"
    fi
}

# CSVヘッダー
echo "Project,Files,TotalTime(ms),AvgTime(ms)" > "$RESULTS_DIR/benchmark_results.csv"

# TypeScriptのベンチマーク（React）
run_benchmark "React-JS" "/tmp/benchmark_repos/react/packages/react/src" "*.js" "typescript"
run_benchmark "React-JSX" "/tmp/benchmark_repos/react/packages/react-dom/src" "*.js" "typescript"

# Rustのベンチマーク（Deno）
run_benchmark "Deno-CLI" "/tmp/benchmark_repos/deno/cli" "*.rs" "rust"
run_benchmark "Deno-Runtime" "/tmp/benchmark_repos/deno/runtime" "*.rs" "rust"

# ストレージベンチマーク
echo -e "\n${GREEN}Running storage benchmarks...${NC}"
cargo bench --bench storage_benchmark -- --quick 2>&1 | grep -E "time:" | head -20 > "$RESULTS_DIR/storage_bench.txt"

# 並列処理ベンチマーク
echo -e "\n${GREEN}Running parallel processing benchmarks...${NC}"
cargo bench --bench storage_benchmark -- save_parallel --quick 2>&1 | grep -E "time:" | head -10 > "$RESULTS_DIR/parallel_bench.txt"

# キャッシュベンチマーク
echo -e "\n${GREEN}Running cache benchmarks...${NC}"
cargo bench --bench cache_benchmark -- cache_hit_rate --quick 2>&1 | grep -E "time:" | head -5 > "$RESULTS_DIR/cache_bench.txt"

# 結果サマリー
echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}           Benchmark Summary            ${NC}"
echo -e "${GREEN}========================================${NC}"

echo -e "\n${YELLOW}Index Generation Results:${NC}"
cat "$RESULTS_DIR/benchmark_results.csv" | column -t -s ','

if [ -f "$RESULTS_DIR/storage_bench.txt" ]; then
    echo -e "\n${YELLOW}Storage Benchmark (sample):${NC}"
    head -5 "$RESULTS_DIR/storage_bench.txt"
fi

if [ -f "$RESULTS_DIR/parallel_bench.txt" ]; then
    echo -e "\n${YELLOW}Parallel Processing (sample):${NC}"
    head -3 "$RESULTS_DIR/parallel_bench.txt"
fi

if [ -f "$RESULTS_DIR/cache_bench.txt" ]; then
    echo -e "\n${YELLOW}Cache Performance:${NC}"
    cat "$RESULTS_DIR/cache_bench.txt"
fi

echo -e "\n${GREEN}Results saved to: $RESULTS_DIR${NC}"
echo -e "${GREEN}========================================${NC}"