# LSIF Indexer パフォーマンス最適化 実装ガイド

## 概要

このガイドでは、LSIF Indexerで検証した各最適化手法の実装方法と、実際のコードへの適用方法を説明します。

## 目次

1. [並列処理の実装](#並列処理の実装)
2. [メモリプールの実装](#メモリプールの実装)
3. [String型インターン化の実装](#string型インターン化の実装)
4. [ロックフリーデータ構造の実装](#ロックフリーデータ構造の実装)
5. [パフォーマンス測定の実装](#パフォーマンス測定の実装)

## 並列処理の実装

### 基本的な並列処理

```rust
use rayon::prelude::*;

// ❌ 悪い例：常に並列処理
pub fn process_files_bad(files: Vec<PathBuf>) -> Vec<Symbol> {
    files.par_iter()
        .flat_map(|file| parse_file(file))
        .collect()
}

// ✅ 良い例：適応的並列処理
pub fn process_files_good(files: Vec<PathBuf>) -> Vec<Symbol> {
    const PARALLEL_THRESHOLD: usize = 50;
    
    if files.len() < PARALLEL_THRESHOLD {
        // シーケンシャル処理
        files.iter()
            .flat_map(|file| parse_file(file))
            .collect()
    } else {
        // 並列処理
        files.par_iter()
            .flat_map(|file| parse_file(file))
            .collect()
    }
}
```

### インクリメンタル更新の並列化

```rust
pub struct IncrementalIndexer {
    thread_pool: rayon::ThreadPool,
}

impl IncrementalIndexer {
    pub fn new() -> Self {
        Self {
            thread_pool: rayon::ThreadPoolBuilder::new()
                .num_threads(num_cpus::get())
                .build()
                .unwrap(),
        }
    }
    
    pub fn update_changed_files(&self, changes: Vec<FileChange>) -> Result<()> {
        // ファイル変更を種類別にグループ化
        let (additions, modifications, deletions) = self.group_changes(changes);
        
        // 並列処理用のスコープ
        self.thread_pool.scope(|s| {
            // 追加ファイルの処理
            s.spawn(|_| {
                additions.par_iter().for_each(|file| {
                    self.index_file(file);
                });
            });
            
            // 変更ファイルの処理
            s.spawn(|_| {
                modifications.par_iter().for_each(|file| {
                    self.reindex_file(file);
                });
            });
            
            // 削除ファイルの処理（シーケンシャル）
            deletions.iter().for_each(|file| {
                self.remove_from_index(file);
            });
        });
        
        Ok(())
    }
}
```

### チャンクベース並列処理

```rust
use rayon::prelude::*;

pub fn process_large_dataset(items: Vec<Item>) -> Vec<Result> {
    const CHUNK_SIZE: usize = 1000;
    
    items
        .par_chunks(CHUNK_SIZE)
        .flat_map(|chunk| {
            // 各チャンクを処理
            chunk.iter().map(process_item).collect::<Vec<_>>()
        })
        .collect()
}
```

## メモリプールの実装

### 基本的なオブジェクトプール

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::VecDeque;

pub struct ObjectPool<T> {
    pool: Arc<RwLock<VecDeque<T>>>,
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    reset: Arc<dyn Fn(&mut T) + Send + Sync>,
    max_size: usize,
}

impl<T> ObjectPool<T> {
    pub fn new<F, R>(factory: F, reset: R, max_size: usize) -> Self 
    where
        F: Fn() -> T + Send + Sync + 'static,
        R: Fn(&mut T) + Send + Sync + 'static,
    {
        Self {
            pool: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            factory: Arc::new(factory),
            reset: Arc::new(reset),
            max_size,
        }
    }
    
    pub fn acquire(&self) -> PooledObject<T> {
        let obj = {
            let mut pool = self.pool.write();
            pool.pop_front().unwrap_or_else(|| (self.factory)())
        };
        
        PooledObject {
            object: Some(obj),
            pool: Arc::clone(&self.pool),
            reset: Arc::clone(&self.reset),
            max_size: self.max_size,
        }
    }
}

pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<RwLock<VecDeque<T>>>,
    reset: Arc<dyn Fn(&mut T) + Send + Sync>,
    max_size: usize,
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(mut obj) = self.object.take() {
            (self.reset)(&mut obj);
            
            let mut pool = self.pool.write();
            if pool.len() < self.max_size {
                pool.push_back(obj);
            }
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}
```

### Symbol専用プール

```rust
use crate::{Symbol, SymbolKind, Range, Position};

pub struct SymbolPool {
    pool: ObjectPool<Symbol>,
}

impl SymbolPool {
    pub fn new(capacity: usize) -> Self {
        let factory = || Symbol {
            id: String::with_capacity(64),
            name: String::with_capacity(128),
            kind: SymbolKind::Function,
            file_path: String::with_capacity(256),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            },
            documentation: None,
        };
        
        let reset = |symbol: &mut Symbol| {
            symbol.id.clear();
            symbol.name.clear();
            symbol.file_path.clear();
            symbol.documentation = None;
            symbol.kind = SymbolKind::Function;
            symbol.range.start.line = 0;
            symbol.range.start.character = 0;
            symbol.range.end.line = 0;
            symbol.range.end.character = 0;
        };
        
        Self {
            pool: ObjectPool::new(factory, reset, capacity),
        }
    }
    
    pub fn create_symbol(&self) -> PooledObject<Symbol> {
        self.pool.acquire()
    }
}

// 使用例
pub fn parse_with_pool(content: &str, pool: &SymbolPool) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    
    for item in parse_items(content) {
        let mut symbol = pool.create_symbol();
        symbol.id = item.id;
        symbol.name = item.name;
        symbol.kind = item.kind;
        // ... 他のフィールドを設定
        
        symbols.push((*symbol).clone()); // クローンして所有権を取得
    }
    
    symbols
}
```

## String型インターン化の実装

### 基本的なStringInterner

```rust
use dashmap::DashMap;
use std::sync::Arc;
use parking_lot::RwLock;

pub struct StringInterner {
    // 文字列 -> ID のマップ
    string_to_id: Arc<DashMap<String, u32>>,
    // ID -> 文字列 のベクター
    id_to_string: Arc<RwLock<Vec<Arc<str>>>>,
    next_id: Arc<AtomicU32>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            string_to_id: Arc::new(DashMap::new()),
            id_to_string: Arc::new(RwLock::new(Vec::new())),
            next_id: Arc::new(AtomicU32::new(0)),
        }
    }
    
    pub fn intern(&self, s: &str) -> InternedString {
        // 既存の文字列をチェック
        if let Some(entry) = self.string_to_id.get(s) {
            return InternedString {
                id: *entry.value(),
                interner: self.clone(),
            };
        }
        
        // 新しい文字列を登録
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let arc_str: Arc<str> = Arc::from(s);
        
        self.string_to_id.insert(s.to_string(), id);
        
        let mut strings = self.id_to_string.write();
        if strings.len() <= id as usize {
            strings.resize(id as usize + 1, Arc::from(""));
        }
        strings[id as usize] = arc_str;
        
        InternedString {
            id,
            interner: self.clone(),
        }
    }
    
    pub fn get(&self, id: u32) -> Option<Arc<str>> {
        let strings = self.id_to_string.read();
        strings.get(id as usize).cloned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InternedString {
    id: u32,
    interner: *const StringInterner, // 参照のみ、所有権なし
}

impl InternedString {
    pub fn as_str(&self) -> &str {
        unsafe {
            let interner = &*self.interner;
            interner.get(self.id)
                .map(|arc| &**arc)
                .unwrap_or("")
        }
    }
}
```

### 最適化されたインターン化Symbol

```rust
pub struct InternedSymbol {
    pub id: InternedString,
    pub name: InternedString,
    pub kind: SymbolKind,
    pub file_path: InternedString,
    pub range: Range,
    pub documentation: Option<InternedString>,
}

impl InternedSymbol {
    pub fn new(interner: &StringInterner, symbol: Symbol) -> Self {
        Self {
            id: interner.intern(&symbol.id),
            name: interner.intern(&symbol.name),
            kind: symbol.kind,
            file_path: interner.intern(&symbol.file_path),
            range: symbol.range,
            documentation: symbol.documentation
                .as_ref()
                .map(|d| interner.intern(d)),
        }
    }
    
    pub fn to_symbol(&self) -> Symbol {
        Symbol {
            id: self.id.as_str().to_string(),
            name: self.name.as_str().to_string(),
            kind: self.kind,
            file_path: self.file_path.as_str().to_string(),
            range: self.range,
            documentation: self.documentation
                .map(|d| d.as_str().to_string()),
        }
    }
}
```

## ロックフリーデータ構造の実装

### 基本的なロックフリーマップ

```rust
use crossbeam_skiplist::SkipMap;
use std::sync::Arc;

pub struct LockFreeMap<K, V> {
    map: Arc<SkipMap<K, V>>,
}

impl<K: Ord, V> LockFreeMap<K, V> {
    pub fn new() -> Self {
        Self {
            map: Arc::new(SkipMap::new()),
        }
    }
    
    pub fn insert(&self, key: K, value: V) {
        self.map.insert(key, value);
    }
    
    pub fn get(&self, key: &K) -> Option<V> 
    where
        V: Clone,
    {
        self.map.get(key).map(|entry| entry.value().clone())
    }
    
    pub fn remove(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.map.remove(key).map(|entry| entry.value().clone())
    }
    
    pub fn update<F>(&self, key: &K, updater: F) -> bool
    where
        F: Fn(&V) -> V,
        V: Clone,
    {
        if let Some(entry) = self.map.get(key) {
            let old_value = entry.value();
            let new_value = updater(old_value);
            self.map.insert(key.clone(), new_value);
            true
        } else {
            false
        }
    }
}
```

### CAS操作の実装

```rust
use std::sync::atomic::{AtomicPtr, Ordering};

pub struct AtomicOption<T> {
    ptr: AtomicPtr<T>,
}

impl<T> AtomicOption<T> {
    pub fn new(value: Option<T>) -> Self {
        let ptr = value
            .map(|v| Box::into_raw(Box::new(v)))
            .unwrap_or(std::ptr::null_mut());
        
        Self {
            ptr: AtomicPtr::new(ptr),
        }
    }
    
    pub fn compare_and_swap(&self, current: Option<&T>, new: Option<T>) -> bool {
        let current_ptr = current
            .map(|v| v as *const T as *mut T)
            .unwrap_or(std::ptr::null_mut());
        
        let new_ptr = new
            .map(|v| Box::into_raw(Box::new(v)))
            .unwrap_or(std::ptr::null_mut());
        
        let result = self.ptr.compare_exchange(
            current_ptr,
            new_ptr,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        
        match result {
            Ok(_) => {
                // 成功：古い値を解放
                if !current_ptr.is_null() {
                    unsafe { Box::from_raw(current_ptr); }
                }
                true
            }
            Err(_) => {
                // 失敗：新しい値を解放
                if !new_ptr.is_null() {
                    unsafe { Box::from_raw(new_ptr); }
                }
                false
            }
        }
    }
    
    pub fn load(&self) -> Option<&T> {
        let ptr = self.ptr.load(Ordering::SeqCst);
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(&*ptr) }
        }
    }
}

impl<T> Drop for AtomicOption<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::SeqCst);
        if !ptr.is_null() {
            unsafe { Box::from_raw(ptr); }
        }
    }
}
```

## パフォーマンス測定の実装

### 簡易プロファイラー

```rust
use std::time::{Duration, Instant};
use std::collections::HashMap;
use parking_lot::RwLock;

pub struct Profiler {
    timings: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            timings: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub fn measure<F, R>(&self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        
        let mut timings = self.timings.write();
        timings.entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(duration);
        
        result
    }
    
    pub fn report(&self) {
        let timings = self.timings.read();
        
        println!("Performance Report:");
        println!("{:-<60}", "");
        
        for (name, durations) in timings.iter() {
            let total: Duration = durations.iter().sum();
            let count = durations.len();
            let avg = total / count as u32;
            
            let min = durations.iter().min().unwrap();
            let max = durations.iter().max().unwrap();
            
            println!("{:<30} | Avg: {:>10.3?} | Min: {:>10.3?} | Max: {:>10.3?} | Count: {}",
                name, avg, min, max, count);
        }
        
        println!("{:-<60}", "");
    }
}

// マクロによる簡単な使用
#[macro_export]
macro_rules! profile {
    ($profiler:expr, $name:expr, $body:expr) => {
        $profiler.measure($name, || $body)
    };
}

// 使用例
pub fn process_with_profiling(files: Vec<PathBuf>, profiler: &Profiler) {
    let symbols = profile!(profiler, "parse_files", {
        parse_all_files(files)
    });
    
    let graph = profile!(profiler, "build_graph", {
        build_code_graph(symbols)
    });
    
    profile!(profiler, "save_to_db", {
        save_graph_to_database(graph)
    });
    
    profiler.report();
}
```

### メモリ使用量の追跡

```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc(layout);
        if !ret.is_null() {
            ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
        }
        ret
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        DEALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

pub fn print_memory_usage() {
    let allocated = ALLOCATED.load(Ordering::SeqCst);
    let deallocated = DEALLOCATED.load(Ordering::SeqCst);
    let current = allocated.saturating_sub(deallocated);
    
    println!("Memory Usage:");
    println!("  Allocated:   {:>10} bytes", allocated);
    println!("  Deallocated: {:>10} bytes", deallocated);
    println!("  Current:     {:>10} bytes ({:.2} MB)", 
        current, current as f64 / 1_048_576.0);
}
```

## テストとベンチマーク

### 統合テスト

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parallel_threshold() {
        // 小規模データセット（閾値未満）
        let small_files = vec![PathBuf::from("a.rs"); 10];
        let result = process_files_good(small_files);
        assert!(!result.is_empty());
        
        // 大規模データセット（閾値以上）
        let large_files = vec![PathBuf::from("b.rs"); 100];
        let result = process_files_good(large_files);
        assert!(!result.is_empty());
    }
    
    #[test]
    fn test_memory_pool_reuse() {
        let pool = SymbolPool::new(10);
        let mut handles = vec![];
        
        // プールからオブジェクトを取得
        for _ in 0..5 {
            handles.push(pool.create_symbol());
        }
        
        // オブジェクトを返却
        handles.clear();
        
        // 再利用されることを確認
        let _reused = pool.create_symbol();
        // プールの統計情報で確認
    }
}
```

### ベンチマーク

```rust
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn benchmark_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimizations");
    
    for size in [100, 1000, 10000].iter() {
        // 標準実装
        group.bench_with_input(
            BenchmarkId::new("standard", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let data = generate_test_data(size);
                    process_standard(black_box(data))
                })
            },
        );
        
        // 並列処理
        group.bench_with_input(
            BenchmarkId::new("parallel", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let data = generate_test_data(size);
                    process_parallel(black_box(data))
                })
            },
        );
        
        // メモリプール
        group.bench_with_input(
            BenchmarkId::new("memory_pool", size),
            size,
            |b, &size| {
                let pool = SymbolPool::new(100);
                b.iter(|| {
                    let data = generate_test_data(size);
                    process_with_pool(black_box(data), &pool)
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, benchmark_optimizations);
criterion_main!(benches);
```

## まとめ

このガイドで紹介した実装パターンは、LSIF Indexerでの検証結果に基づいています。重要なポイント：

1. **測定してから最適化する**
2. **シンプルな実装から始める**
3. **実際のワークロードで検証する**
4. **保守性とのバランスを考慮する**

各最適化手法には適切な使用場面があり、すべてのケースで有効というわけではありません。プロジェクトの要件に応じて、適切な手法を選択してください。