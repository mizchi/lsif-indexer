use dashmap::DashMap;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::fmt;
use std::sync::Arc;

/// インターン化された文字列を表す軽量な識別子
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedString(u32);

impl InternedString {
    /// インターン化された文字列を取得
    pub fn as_str(&self) -> &'static str {
        GLOBAL_INTERNER.get(*self)
    }

    /// 内部のIDを取得
    pub fn id(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl AsRef<str> for InternedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// スレッドセーフな文字列インターナー
pub struct StringInterner {
    /// 文字列 -> ID のマッピング
    string_to_id: Arc<DashMap<String, u32>>,
    /// ID -> 文字列 のマッピング（静的生存期間のためにリーク）
    id_to_string: Arc<RwLock<Vec<&'static str>>>,
    /// 統計情報
    stats: Arc<RwLock<InternerStats>>,
}

#[derive(Default, Debug, Clone)]
pub struct InternerStats {
    pub total_strings: usize,
    pub total_bytes: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl StringInterner {
    /// 新しいインターナーを作成
    pub fn new() -> Self {
        Self {
            string_to_id: Arc::new(DashMap::new()),
            id_to_string: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(InternerStats::default())),
        }
    }

    /// 文字列をインターン化
    pub fn intern(&self, s: &str) -> InternedString {
        let mut stats = self.stats.write();

        // 既存の文字列を検索
        if let Some(id) = self.string_to_id.get(s) {
            stats.cache_hits += 1;
            return InternedString(*id.value());
        }

        stats.cache_misses += 1;

        // 新しい文字列を追加
        let mut id_to_string = self.id_to_string.write();
        let id = id_to_string.len() as u32;

        // 文字列をリークして'static生存期間を得る
        let leaked_str: &'static str = Box::leak(s.to_string().into_boxed_str());

        id_to_string.push(leaked_str);
        self.string_to_id.insert(s.to_string(), id);

        stats.total_strings += 1;
        stats.total_bytes += s.len();

        InternedString(id)
    }

    /// バッチでインターン化
    pub fn intern_batch(&self, strings: Vec<String>) -> Vec<InternedString> {
        strings.iter().map(|s| self.intern(s)).collect()
    }

    /// IDから文字列を取得
    pub fn get(&self, interned: InternedString) -> &'static str {
        let id_to_string = self.id_to_string.read();
        id_to_string.get(interned.0 as usize).copied().unwrap_or("")
    }

    /// 統計情報を取得
    pub fn stats(&self) -> InternerStats {
        self.stats.read().clone()
    }

    /// インターン化された文字列の数
    pub fn len(&self) -> usize {
        self.id_to_string.read().len()
    }

    /// インターナーが空かどうかを確認
    pub fn is_empty(&self) -> bool {
        self.id_to_string.read().is_empty()
    }

    /// メモリ使用量の推定値（バイト単位）
    pub fn estimated_memory_usage(&self) -> usize {
        let stats = self.stats.read();
        let overhead_per_string = 64; // HashMap/Vec のオーバーヘッド推定値
        stats.total_bytes + (stats.total_strings * overhead_per_string)
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// グローバルなインターナー
static GLOBAL_INTERNER: Lazy<StringInterner> = Lazy::new(StringInterner::new);

/// グローバルインターナーで文字列をインターン化
pub fn intern(s: &str) -> InternedString {
    GLOBAL_INTERNER.intern(s)
}

/// グローバルインターナーの統計情報を取得
pub fn interner_stats() -> InternerStats {
    GLOBAL_INTERNER.stats()
}

/// インターン化されたSymbol構造体
#[derive(Debug, Clone, PartialEq)]
pub struct InternedSymbol {
    pub id: InternedString,
    pub name: InternedString,
    pub kind: crate::SymbolKind,
    pub file_path: InternedString,
    pub range: crate::Range,
    pub documentation: Option<InternedString>,
    pub detail: Option<InternedString>,
}

impl InternedSymbol {
    /// 通常のSymbolから作成
    pub fn from_symbol(symbol: crate::Symbol) -> Self {
        Self {
            id: intern(&symbol.id),
            name: intern(&symbol.name),
            kind: symbol.kind,
            file_path: intern(&symbol.file_path),
            range: symbol.range,
            documentation: symbol.documentation.as_deref().map(intern),
            detail: symbol.detail.as_deref().map(intern),
        }
    }

    /// 通常のSymbolに変換
    pub fn to_symbol(&self) -> crate::Symbol {
        crate::Symbol {
            id: self.id.as_str().to_string(),
            name: self.name.as_str().to_string(),
            kind: self.kind,
            file_path: self.file_path.as_str().to_string(),
            range: self.range,
            documentation: self.documentation.map(|d| d.as_str().to_string()),
            detail: self.detail.map(|d| d.as_str().to_string()),
        }
    }

    /// メモリ使用量の推定値
    pub fn estimated_size(&self) -> usize {
        // InternedStringは4バイト、その他の固定サイズフィールド
        4 * 3 + // id, name, file_path
        std::mem::size_of::<crate::SymbolKind>() +
        std::mem::size_of::<crate::Range>() +
        8 // 2 * Option<InternedString> (documentation, detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interner_basic() {
        let interner = StringInterner::new();

        let s1 = interner.intern("hello");
        let s2 = interner.intern("world");
        let s3 = interner.intern("hello"); // 同じ文字列

        assert_eq!(s1, s3); // 同じIDを持つ
        assert_ne!(s1, s2);

        // ローカルインターナーを使う場合は、getメソッドを直接使う
        assert_eq!(interner.get(s1), "hello");
        assert_eq!(interner.get(s2), "world");

        let stats = interner.stats();
        assert_eq!(stats.total_strings, 2); // "hello"と"world"のみ
        assert_eq!(stats.cache_hits, 1); // "hello"の2回目
    }

    #[test]
    fn test_global_interner() {
        let s1 = intern("test1");
        let s2 = intern("test2");
        let s3 = intern("test1");

        assert_eq!(s1, s3);
        assert_ne!(s1, s2);

        assert_eq!(s1.as_str(), "test1");
    }

    #[test]
    fn test_interned_symbol() {
        let symbol = crate::Symbol {
            id: "sym_1".to_string(),
            name: "function1".to_string(),
            kind: crate::SymbolKind::Function,
            file_path: "src/main.rs".to_string(),
            range: crate::Range {
                start: crate::Position {
                    line: 0,
                    character: 0,
                },
                end: crate::Position {
                    line: 1,
                    character: 0,
                },
            },
            documentation: Some("Test doc".to_string()),
            detail: None,
        };

        let interned = InternedSymbol::from_symbol(symbol.clone());
        let converted = interned.to_symbol();

        assert_eq!(symbol, converted);

        // メモリサイズが大幅に削減されているはず
        let original_size = std::mem::size_of::<crate::Symbol>()
            + symbol.id.len()
            + symbol.name.len()
            + symbol.file_path.len()
            + 8;
        let interned_size = interned.estimated_size();

        assert!(interned_size < original_size);
    }

    #[test]
    fn test_batch_intern() {
        let interner = StringInterner::new();

        let strings = vec![
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
            "alpha".to_string(), // 重複
        ];

        let interned = interner.intern_batch(strings);

        assert_eq!(interned.len(), 4);
        assert_eq!(interned[0], interned[3]); // 同じ文字列は同じID
        assert_eq!(interner.len(), 3); // ユニークな文字列は3つ
    }

    #[test]
    fn test_memory_estimation() {
        let interner = StringInterner::new();

        for i in 0..100 {
            interner.intern(&format!("string_{}", i));
        }

        let memory = interner.estimated_memory_usage();
        assert!(memory > 0);

        let stats = interner.stats();
        assert_eq!(stats.total_strings, 100);
        assert_eq!(stats.cache_misses, 100);
        assert_eq!(stats.cache_hits, 0);
    }
}
