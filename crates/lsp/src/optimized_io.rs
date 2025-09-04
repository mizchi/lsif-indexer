use anyhow::Result;
use memmap2::Mmap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// 最適化されたファイル読み込み
pub struct OptimizedFileReader {
    /// メモリマップを使用する閾値（バイト）
    mmap_threshold: usize,
}

impl Default for OptimizedFileReader {
    fn default() -> Self {
        Self {
            // 1MB以上のファイルはメモリマップを使用
            mmap_threshold: 1024 * 1024,
        }
    }
}

impl OptimizedFileReader {
    pub fn new(mmap_threshold: usize) -> Self {
        Self { mmap_threshold }
    }

    /// ファイルを最適な方法で読み込む
    pub fn read_file(&self, path: &Path) -> Result<String> {
        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len() as usize;

        if file_size > self.mmap_threshold {
            // 大きいファイルはメモリマップを使用
            self.read_with_mmap(path)
        } else {
            // 小さいファイルはバッファードリーダーを使用
            self.read_with_buffer(path)
        }
    }

    /// ファイルを行ごとに処理（メモリ効率的）
    pub fn process_lines<F>(&self, path: &Path, mut processor: F) -> Result<()>
    where
        F: FnMut(&str, usize) -> Result<()>,
    {
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(64 * 1024, file); // 64KBバッファ

        for (line_num, line) in reader.lines().enumerate() {
            processor(&line?, line_num)?;
        }

        Ok(())
    }

    /// バッファードリーダーでファイルを読む
    fn read_with_buffer(&self, path: &Path) -> Result<String> {
        let file = File::open(path)?;
        let mut reader = BufReader::with_capacity(32 * 1024, file); // 32KBバッファ
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        Ok(content)
    }

    /// メモリマップでファイルを読む
    fn read_with_mmap(&self, path: &Path) -> Result<String> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        // UTF-8として解釈
        let content = std::str::from_utf8(&mmap)?;
        Ok(content.to_string())
    }

    /// 複数ファイルを並列で読み込み
    pub fn read_files_parallel(&self, paths: &[&Path]) -> Vec<Result<String>> {
        use rayon::prelude::*;

        paths.par_iter().map(|path| self.read_file(path)).collect()
    }
}

/// ファイル内容のキャッシュ
pub struct FileContentCache {
    cache: dashmap::DashMap<std::path::PathBuf, (String, std::time::SystemTime)>,
    max_cache_size: usize,
}

impl Default for FileContentCache {
    fn default() -> Self {
        Self {
            cache: dashmap::DashMap::new(),
            max_cache_size: 100, // デフォルトで100ファイルまでキャッシュ
        }
    }
}

impl FileContentCache {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            cache: dashmap::DashMap::new(),
            max_cache_size,
        }
    }

    /// キャッシュからファイルを取得、なければ読み込んでキャッシュ
    pub fn get_or_read(&self, path: &Path) -> Result<String> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;

        // キャッシュチェック
        if let Some(entry) = self.cache.get(path) {
            if entry.1 == modified {
                // キャッシュが有効
                return Ok(entry.0.clone());
            }
        }

        // ファイルを読み込み
        let reader = OptimizedFileReader::default();
        let content = reader.read_file(path)?;

        // キャッシュサイズチェック
        if self.cache.len() >= self.max_cache_size {
            // 古いエントリを削除（簡易的にランダムに1つ削除）
            if let Some(entry) = self.cache.iter().next() {
                self.cache.remove(entry.key());
            }
        }

        // キャッシュに保存
        self.cache
            .insert(path.to_path_buf(), (content.clone(), modified));

        Ok(content)
    }

    /// キャッシュをクリア
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// キャッシュサイズを取得
    pub fn size(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_small_file_reading() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello, World!").unwrap();
        writeln!(file, "This is a test file.").unwrap();

        let reader = OptimizedFileReader::default();
        let content = reader.read_file(file.path()).unwrap();

        assert!(content.contains("Hello, World!"));
        assert!(content.contains("This is a test file."));
    }

    #[test]
    fn test_line_processing() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let reader = OptimizedFileReader::default();
        let mut lines = Vec::new();

        reader
            .process_lines(file.path(), |line, num| {
                lines.push(format!("{}: {}", num, line));
                Ok(())
            })
            .unwrap();

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "0: Line 1");
        assert_eq!(lines[1], "1: Line 2");
        assert_eq!(lines[2], "2: Line 3");
    }

    #[test]
    fn test_file_cache() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Cached content").unwrap();
        file.flush().unwrap();

        let cache = FileContentCache::new(10);

        // 初回読み込み
        let content1 = cache.get_or_read(file.path()).unwrap();
        assert_eq!(content1.trim(), "Cached content");

        // キャッシュから取得
        let content2 = cache.get_or_read(file.path()).unwrap();
        assert_eq!(content1, content2);

        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn test_cache_invalidation() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        // 初回の内容を書き込み
        std::fs::write(&path, "Original content\n").unwrap();

        let cache = FileContentCache::new(10);

        // 初回読み込み
        let content1 = cache.get_or_read(&path).unwrap();
        assert_eq!(content1.trim(), "Original content");

        // ファイルを更新（タイムスタンプを変更するため少し待つ）
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&path, "Updated content\n").unwrap();

        // キャッシュが無効化され、新しい内容が読まれる
        let content2 = cache.get_or_read(&path).unwrap();
        assert_eq!(content2.trim(), "Updated content");
    }

    #[test]
    fn test_parallel_file_reading() {
        let mut files = Vec::new();
        let mut paths = Vec::new();

        for i in 0..5 {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, "File {}", i).unwrap();
            paths.push(file.path().to_path_buf());
            files.push(file);
        }

        let reader = OptimizedFileReader::default();
        let path_refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        let results = reader.read_files_parallel(&path_refs);

        assert_eq!(results.len(), 5);
        for (i, result) in results.iter().enumerate() {
            assert!(result.as_ref().unwrap().contains(&format!("File {}", i)));
        }
    }

    #[test]
    fn test_performance_improvement() {
        use std::time::Instant;

        // テスト用の大きめのファイルを作成
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..10000 {
            writeln!(
                file,
                "Line {}: Some test content that is reasonably long",
                i
            )
            .unwrap();
        }
        file.flush().unwrap();

        // 通常の読み込み
        let start = Instant::now();
        for _ in 0..10 {
            let _ = std::fs::read_to_string(file.path());
        }
        let normal_time = start.elapsed();

        // 最適化された読み込み（キャッシュあり）
        let cache = FileContentCache::new(10);
        let start = Instant::now();
        for _ in 0..10 {
            let _ = cache.get_or_read(file.path());
        }
        let optimized_time = start.elapsed();

        // キャッシュありの方が高速であることを確認（通常は2倍以上高速だが、環境により変動するため1.5倍で判定）
        assert!(
            optimized_time < normal_time * 2 / 3,
            "Cache should be faster. Normal: {:?}, Optimized: {:?}",
            normal_time,
            optimized_time
        );
    }
}
