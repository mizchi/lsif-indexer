#![allow(dead_code)]

use anyhow::{Context, Result};
use std::path::Path;
use lsif_indexer::cli::storage::IndexStorage;
use lsif_indexer::cli::differential_indexer::DifferentialIndexer;

pub fn execute_restore(db: &Path, target_commit: Option<String>, dry_run: bool) -> Result<()> {
    println!("🔄 Restoring index from metadata...");
    
    // Load current metadata
    let storage = IndexStorage::open(db)?;
    let metadata = storage.load_metadata()
        .context("Failed to load metadata")?
        .context("No metadata found. Run 'lsif diff' first to create metadata")?;
    
    println!("📊 Current metadata:");
    println!("  - Last indexed: {}", metadata.created_at);
    println!("  - Git commit: {:?}", metadata.git_commit_hash);
    println!("  - Files: {}", metadata.files_count);
    println!("  - Symbols: {}", metadata.symbols_count);
    println!("  - File hashes: {}", metadata.file_hashes.len());
    
    if dry_run {
        println!("\n🔍 Dry run mode - showing what would be done:");
        
        // Check current git status
        if let Ok(repo) = git2::Repository::open(".") {
            if let Ok(head) = repo.head() {
                if let Some(oid) = head.target() {
                    let current_commit = oid.to_string();
                    
                    if let Some(saved_commit) = &metadata.git_commit_hash {
                        if &current_commit != saved_commit {
                            println!("  ⚠️  Git HEAD has changed:");
                            println!("     Saved: {}", saved_commit);
                            println!("     Current: {}", current_commit);
                            println!("  → Would re-index changed files");
                        } else {
                            println!("  ✅ Git HEAD matches saved metadata");
                        }
                    }
                }
            }
        }
        
        // Check file changes based on content hash
        let mut changed_files = 0;
        for (file_path, saved_hash) in &metadata.file_hashes {
            let path = std::path::Path::new(file_path);
            if path.exists() {
                // Calculate current hash
                let detector = lsif_indexer::cli::git_diff::GitDiffDetector::new(".")?;
                if let Ok(current_hash) = detector.calculate_file_hash(path) {
                    if current_hash != *saved_hash {
                        changed_files += 1;
                        if changed_files <= 10 {
                            println!("  📝 Changed: {}", file_path);
                        }
                    }
                }
            } else {
                println!("  ❌ Deleted: {}", file_path);
            }
        }
        
        if changed_files > 10 {
            println!("  ... and {} more changed files", changed_files - 10);
        }
        
        if changed_files > 0 {
            println!("\n  → Would re-index {} changed files", changed_files);
        } else {
            println!("\n  ✅ No changes detected");
        }
        
        return Ok(());
    }
    
    // Perform actual restore
    println!("\n🚀 Performing restore...");
    
    // If target commit specified, checkout to that commit first
    if let Some(commit) = target_commit {
        println!("  Checking out commit: {}", commit);
        // Note: This would require git checkout, which should be done manually
        println!("  ⚠️  Please run 'git checkout {}' manually first", commit);
        return Ok(());
    }
    
    // Run differential indexing based on saved metadata
    let project_root = std::env::current_dir()?;
    let mut indexer = DifferentialIndexer::new(db, &project_root)?;
    
    let result = indexer.index_differential()?;
    
    println!("\n✅ Restore complete!");
    println!("┌─────────────────────────────────────┐");
    println!("│ Files:                              │");
    println!("│   Added:        {:4}                │", result.files_added);
    println!("│   Modified:     {:4}                │", result.files_modified);
    println!("│   Deleted:      {:4}                │", result.files_deleted);
    println!("├─────────────────────────────────────┤");
    println!("│ Symbols:                            │");
    println!("│   Added:       {:5}                │", result.symbols_added);
    println!("│   Updated:     {:5}                │", result.symbols_updated);
    println!("│   Deleted:     {:5}                │", result.symbols_deleted);
    println!("├─────────────────────────────────────┤");
    println!("│ Time: {:.2}s                         │", result.duration.as_secs_f64());
    println!("└─────────────────────────────────────┘");
    
    Ok(())
}