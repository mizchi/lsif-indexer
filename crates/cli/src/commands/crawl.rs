use super::utils::{print_info, print_success, print_warning};
use crate::definition_crawler::DefinitionCrawler;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn handle_crawl(
    db_path: &str,
    project_root: &str,
    files: Vec<String>,
    max_depth: u32,
    max_files: usize,
    show_progress: bool,
) -> Result<()> {
    let start = Instant::now();

    // 開始ファイルを決定
    let start_files = if files.is_empty() {
        // 現在のディレクトリから適切なファイルを検出
        detect_entry_points(Path::new(project_root))?
    } else {
        // 指定されたファイルを使用
        files
            .into_iter()
            .map(|f| {
                let path = Path::new(&f);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    Path::new(project_root).join(path)
                }
            })
            .collect()
    };

    if start_files.is_empty() {
        print_warning("No entry point files found. Please specify files to crawl.");
        return Ok(());
    }

    print_info(
        &format!(
            "Starting smart crawl from {} files (depth: {}, max: {} files)",
            start_files.len(),
            max_depth,
            max_files
        ),
        "🕷️",
    );

    // クローラーを初期化
    let mut crawler = DefinitionCrawler::new(
        Path::new(db_path),
        Path::new(project_root),
        max_depth,
        max_files,
    )?;

    // プログレス表示の設定
    if show_progress {
        print_info("Crawling with progress tracking enabled", "📊");
    }

    // クロール実行
    let stats = if start_files.len() == 1 {
        crawler.crawl_from_file(&start_files[0])?
    } else {
        crawler.crawl_from_files(start_files)?
    };

    // 結果表示
    let elapsed = start.elapsed();
    print_success(&format!(
        "Crawl completed in {:.2}s: {} files indexed, {} symbols found",
        elapsed.as_secs_f64(),
        stats.files_indexed,
        stats.symbols_found
    ));

    if stats.files_failed > 0 {
        print_warning(&format!("{} files failed to index", stats.files_failed));
    }

    // 統計情報を表示
    let symbols_per_file = if stats.files_indexed > 0 {
        stats.symbols_found as f64 / stats.files_indexed as f64
    } else {
        0.0
    };

    print_info(
        &format!("Average symbols per file: {:.1}", symbols_per_file),
        "📊",
    );

    Ok(())
}

/// プロジェクトのエントリーポイントを自動検出
fn detect_entry_points(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut entry_points = Vec::new();

    // main.rs, lib.rs, index.ts, main.py などの一般的なエントリーポイントを検索
    let common_entry_files = [
        "src/main.rs",
        "src/lib.rs",
        "main.rs",
        "lib.rs",
        "src/index.ts",
        "src/index.js",
        "index.ts",
        "index.js",
        "main.py",
        "__main__.py",
        "src/main.go",
        "main.go",
    ];

    for entry_file in &common_entry_files {
        let path = project_root.join(entry_file);
        if path.exists() {
            entry_points.push(path);
            if entry_points.len() >= 3 {
                break; // 最初の3つで十分
            }
        }
    }

    // binディレクトリも確認（Rustプロジェクト）
    let bin_dir = project_root.join("src/bin");
    if bin_dir.exists() && bin_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    entry_points.push(path);
                    if entry_points.len() >= 5 {
                        break;
                    }
                }
            }
        }
    }

    // 現在開いているファイル（環境変数から取得を試みる）
    if let Ok(current_file) = std::env::var("LSIF_CURRENT_FILE") {
        let path = Path::new(&current_file);
        if path.exists() {
            entry_points.insert(0, path.to_path_buf());
        }
    }

    Ok(entry_points)
}
