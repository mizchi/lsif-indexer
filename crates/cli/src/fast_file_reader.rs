use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// ファイルコンテンツのキャッシュ
pub struct FileContentCache {
    cache: HashMap<String, Vec<u8>>,
}

impl Default for FileContentCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FileContentCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// 複数ファイルを並列で事前読み込み
    pub fn preload_files<P: AsRef<Path> + Send + Sync>(&mut self, paths: &[P]) -> Result<()> {
        let contents: Vec<(String, Vec<u8>)> = paths
            .par_iter()
            .filter_map(|path| {
                let path = path.as_ref();
                let path_str = path.to_string_lossy().to_string();

                match fs::read(path) {
                    Ok(content) => Some((path_str, content)),
                    Err(_) => None,
                }
            })
            .collect();

        for (path, content) in contents {
            self.cache.insert(path, content);
        }

        Ok(())
    }

    /// キャッシュからファイル内容を取得
    pub fn get(&self, path: &Path) -> Option<&[u8]> {
        let path_str = path.to_string_lossy();
        self.cache.get(path_str.as_ref()).map(|v| v.as_slice())
    }

    /// キャッシュから文字列として取得
    pub fn get_string(&self, path: &Path) -> Option<String> {
        self.get(path)
            .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
    }
}

/// 高速ファイル読み込みユーティリティ
pub struct FastFileReader;

impl FastFileReader {
    /// バッファサイズを最適化したファイル読み込み
    pub fn read_file_optimized(path: &Path) -> Result<String> {
        // 64KBのバッファサイズ（一般的なL1キャッシュサイズを考慮）
        const BUFFER_SIZE: usize = 64 * 1024;

        let file = fs::File::open(path)?;
        let file_size = file.metadata()?.len() as usize;

        // 小さいファイルは一度に読み込み
        if file_size < BUFFER_SIZE {
            return Ok(fs::read_to_string(path)?);
        }

        // 大きいファイルはバッファリング
        let reader = BufReader::with_capacity(BUFFER_SIZE, file);
        let mut content = String::with_capacity(file_size);

        for line in reader.lines() {
            content.push_str(&line?);
            content.push('\n');
        }

        Ok(content)
    }

    /// 複数ファイルを並列で読み込み
    pub fn read_files_parallel<P: AsRef<Path> + Send + Sync>(
        paths: &[P],
    ) -> Vec<(String, Result<String>)> {
        paths
            .par_iter()
            .map(|path| {
                let path = path.as_ref();
                let path_str = path.to_string_lossy().to_string();
                let content = Self::read_file_optimized(path);
                (path_str, content)
            })
            .collect()
    }
}
