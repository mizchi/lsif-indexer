#!/bin/bash
set -e

echo "======================================"
echo "  差分インデックス効率化ベンチマーク"
echo "======================================"

# テストプロジェクトの準備
TEST_DIR="/tmp/incremental_test"
rm -rf $TEST_DIR
mkdir -p $TEST_DIR

# サンプルファイルを生成
echo "サンプルプロジェクトを生成中..."
for i in {1..100}; do
    cat > "$TEST_DIR/file_$i.rs" << EOF
pub struct Module$i {
    pub value: i32,
}

impl Module$i {
    pub fn new(v: i32) -> Self {
        Self { value: v }
    }
    
    pub fn process(&self) -> i32 {
        self.value * 2
    }
}
EOF
done

echo "======================================"
echo "1. 初回インデックス作成（100ファイル）"
echo "======================================"

START=$(date +%s%N)
for file in $TEST_DIR/*.rs; do
    ./target/release/lsif-indexer generate \
        --source "$file" \
        --output "/tmp/index_full.db" \
        --language rust 2>&1 | grep -q "Index generated" || true
done
END=$(date +%s%N)
FULL_TIME=$((($END - $START) / 1000000))
echo "フルインデックス作成時間: ${FULL_TIME}ms"

echo ""
echo "======================================"
echo "2. 10%のファイルを変更"
echo "======================================"

# 10ファイルを変更
for i in {1..10}; do
    echo "// Modified at $(date)" >> "$TEST_DIR/file_$i.rs"
done

echo "======================================"
echo "3. 差分更新シミュレーション"
echo "======================================"

# 従来の方法（全ファイル再インデックス）
echo "従来方式（全ファイル再処理）:"
START=$(date +%s%N)
for file in $TEST_DIR/*.rs; do
    ./target/release/lsif-indexer generate \
        --source "$file" \
        --output "/tmp/index_traditional.db" \
        --language rust 2>&1 | grep -q "Index generated" || true
done
END=$(date +%s%N)
TRADITIONAL_TIME=$((($END - $START) / 1000000))
echo "  時間: ${TRADITIONAL_TIME}ms"

# 差分更新（変更されたファイルのみ）
echo ""
echo "差分更新方式（変更ファイルのみ）:"
START=$(date +%s%N)
for i in {1..10}; do
    ./target/release/lsif-indexer generate \
        --source "$TEST_DIR/file_$i.rs" \
        --output "/tmp/index_incremental.db" \
        --language rust 2>&1 | grep -q "Index generated" || true
done
END=$(date +%s%N)
INCREMENTAL_TIME=$((($END - $START) / 1000000))
echo "  時間: ${INCREMENTAL_TIME}ms"

echo ""
echo "======================================"
echo "4. 結果サマリー"
echo "======================================"

SPEEDUP=$(echo "scale=2; $TRADITIONAL_TIME / $INCREMENTAL_TIME" | bc)
SAVED_TIME=$(($TRADITIONAL_TIME - $INCREMENTAL_TIME))
SAVED_PERCENT=$(echo "scale=1; ($SAVED_TIME * 100) / $TRADITIONAL_TIME" | bc)

echo "初回フルインデックス: ${FULL_TIME}ms (100ファイル)"
echo "従来の再インデックス: ${TRADITIONAL_TIME}ms (100ファイル)"
echo "差分インデックス:     ${INCREMENTAL_TIME}ms (10ファイル)"
echo ""
echo "効率化:"
echo "  高速化: ${SPEEDUP}倍"
echo "  時間削減: ${SAVED_TIME}ms (${SAVED_PERCENT}%)"
echo ""

# 実プロジェクトでの推定
echo "======================================"
echo "5. 実プロジェクトでの推定効果"
echo "======================================"

echo "React (4,222ファイル)の場合:"
REACT_FULL=$(echo "scale=0; 4222 * $FULL_TIME / 100 / 1000" | bc)
REACT_INCREMENTAL=$(echo "scale=0; 422 * $INCREMENTAL_TIME / 10 / 1000" | bc)
echo "  フルインデックス: 約${REACT_FULL}秒"
echo "  10%変更時の差分: 約${REACT_INCREMENTAL}秒"
echo "  削減時間: $(($REACT_FULL - $REACT_INCREMENTAL))秒"

echo ""
echo "Deno (593ファイル)の場合:"
DENO_FULL=$(echo "scale=0; 593 * $FULL_TIME / 100 / 1000" | bc)
DENO_INCREMENTAL=$(echo "scale=0; 59 * $INCREMENTAL_TIME / 10 / 1000" | bc)
echo "  フルインデックス: 約${DENO_FULL}秒"
echo "  10%変更時の差分: 約${DENO_INCREMENTAL}秒"
echo "  削減時間: $(($DENO_FULL - $DENO_INCREMENTAL))秒"

echo ""
echo "======================================"