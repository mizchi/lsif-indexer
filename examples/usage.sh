#!/bin/bash
# LSIFインデクサーの使用例

echo "=== 基本的な使い方 ==="

# 1. Rustプロジェクトをインデックス化
echo "Rustプロジェクトのインデックス化:"
lsif --files="src/**/*.rs" --output=rust_index.db

# 2. 特定ディレクトリのみインデックス化
echo -e "\n特定ディレクトリのインデックス化:"
lsif --files="src/core/*.rs" --output=core_index.db

# 3. 除外パターンを使用
echo -e "\n除外パターンの使用:"
lsif --files="**/*.rs" --exclude=target --exclude=tests --output=filtered_index.db

# 4. 並列処理とキャッシュの設定
echo -e "\n並列処理の設定:"
lsif --files="src/**/*.rs" --parallel --cache --threads=4 --batch-size=50 --output=optimized_index.db

# 5. サポート言語の一覧表示
echo -e "\nサポート言語の一覧:"
lsif list

# 6. データベースの統計情報
echo -e "\nデータベース統計:"
lsif stats --db=rust_index.db

# 7. クエリ実行例
echo -e "\nクエリ実行:"
# 定義を検索
lsif query --db=rust_index.db definition src/core/mod.rs 1 1

# 8. TypeScript/JavaScript プロジェクト (将来的にサポート予定)
# lsif --files="**/*.{ts,tsx,js,jsx}" --language=typescript --output=ts_index.db

# 9. Python プロジェクト (将来的にサポート予定)
# lsif --files="**/*.py" --language=python --output=py_index.db

echo -e "\n=== 完了 ==="#