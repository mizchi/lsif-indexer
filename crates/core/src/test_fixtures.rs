/// テスト用の共通フィクスチャとビルダー
use crate::{CodeGraph, Symbol, SymbolKind, Range, Position, EdgeKind};

/// テスト用のグラフビルダー
pub struct TestGraphBuilder {
    graph: CodeGraph,
    symbol_counter: usize,
}

impl TestGraphBuilder {
    /// 新しいテストグラフビルダーを作成
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::new(),
            symbol_counter: 0,
        }
    }

    /// デフォルトのシンボルを追加
    pub fn with_symbols(mut self, count: usize) -> Self {
        for i in 0..count {
            let symbol = self.create_test_symbol(
                &format!("symbol_{}", i),
                SymbolKind::Function,
                &format!("test_file_{}.rs", i % 3),
                i as u32,
            );
            self.graph.add_symbol(symbol);
            self.symbol_counter += 1;
        }
        self
    }

    /// 特定の種類のシンボルを追加
    pub fn with_typed_symbols(mut self, kind: SymbolKind, count: usize) -> Self {
        for i in 0..count {
            let symbol = self.create_test_symbol(
                &format!("{:?}_{}", kind, self.symbol_counter + i),
                kind,
                "test.rs",
                (self.symbol_counter + i) as u32,
            );
            self.graph.add_symbol(symbol);
        }
        self.symbol_counter += count;
        self
    }

    /// エッジを追加
    pub fn with_edge(mut self, from_id: &str, to_id: &str, kind: EdgeKind) -> Self {
        // まずシンボルが存在することを確認し、なければ作成
        if !self.graph.get_all_symbols().any(|s| s.id == from_id) {
            let symbol = self.create_test_symbol(from_id, SymbolKind::Function, "test.rs", 0);
            self.graph.add_symbol(symbol);
        }
        if !self.graph.get_all_symbols().any(|s| s.id == to_id) {
            let symbol = self.create_test_symbol(to_id, SymbolKind::Function, "test.rs", 1);
            self.graph.add_symbol(symbol);
        }
        
        // NodeIndexを取得してエッジを追加
        if let (Some(from_idx), Some(to_idx)) = (
            self.graph.get_node_index(from_id),
            self.graph.get_node_index(to_id)
        ) {
            self.graph.add_edge(from_idx, to_idx, kind);
        }
        self
    }

    /// 複数のエッジを追加
    pub fn with_edges(mut self, edges: Vec<(&str, &str, EdgeKind)>) -> Self {
        for (from, to, kind) in edges {
            self = self.with_edge(from, to, kind);
        }
        self
    }

    /// 階層構造を追加
    pub fn with_hierarchy(mut self, parent: &str, children: Vec<&str>) -> Self {
        let parent_symbol = self.create_test_symbol(
            parent,
            SymbolKind::Class,
            "hierarchy.rs",
            0,
        );
        self.graph.add_symbol(parent_symbol);

        for (i, child) in children.iter().enumerate() {
            let child_symbol = self.create_test_symbol(
                child,
                SymbolKind::Method,
                "hierarchy.rs",
                (i + 1) as u32,
            );
            self.graph.add_symbol(child_symbol);
            // NodeIndexを取得してエッジを追加
            if let (Some(child_idx), Some(parent_idx)) = (
                self.graph.get_node_index(child),
                self.graph.get_node_index(parent)
            ) {
                self.graph.add_edge(child_idx, parent_idx, EdgeKind::Definition);
            }
        }
        self
    }

    /// 循環参照を追加
    pub fn with_cycle(mut self, nodes: Vec<&str>) -> Self {
        if nodes.len() < 2 {
            return self;
        }

        // ノードを作成
        for (i, node) in nodes.iter().enumerate() {
            let symbol = self.create_test_symbol(
                node,
                SymbolKind::Function,
                "cycle.rs",
                i as u32,
            );
            self.graph.add_symbol(symbol);
        }

        // 循環エッジを作成
        for i in 0..nodes.len() {
            let from = nodes[i];
            let to = nodes[(i + 1) % nodes.len()];
            // NodeIndexを取得してエッジを追加
            if let (Some(from_idx), Some(to_idx)) = (
                self.graph.get_node_index(from),
                self.graph.get_node_index(to)
            ) {
                self.graph.add_edge(from_idx, to_idx, EdgeKind::Reference);
            }
        }
        self
    }

    /// グラフを構築
    pub fn build(self) -> CodeGraph {
        self.graph
    }

    /// テスト用シンボルを作成（ヘルパーメソッド）
    fn create_test_symbol(
        &self,
        id: &str,
        kind: SymbolKind,
        file_path: &str,
        line: u32,
    ) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            file_path: file_path.to_string(),
            range: Range {
                start: Position { line, character: 0 },
                end: Position { line, character: 10 },
            },
            documentation: Some(format!("Test documentation for {}", id)),
            detail: None,
        }
    }
}

impl Default for TestGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// よく使用するテストグラフのプリセット
pub struct TestGraphPresets;

impl TestGraphPresets {
    /// 小規模なグラフ（5シンボル、3エッジ）
    pub fn small() -> CodeGraph {
        TestGraphBuilder::new()
            .with_symbols(5)
            .with_edge("symbol_0", "symbol_1", EdgeKind::Reference)
            .with_edge("symbol_1", "symbol_2", EdgeKind::Definition)
            .with_edge("symbol_2", "symbol_0", EdgeKind::Reference)
            .build()
    }

    /// 中規模なグラフ（20シンボル、15エッジ）
    pub fn medium() -> CodeGraph {
        let mut builder = TestGraphBuilder::new().with_symbols(20);
        
        for i in 0..15 {
            builder = builder.with_edge(
                &format!("symbol_{}", i),
                &format!("symbol_{}", (i + 1) % 20),
                if i % 2 == 0 { EdgeKind::Reference } else { EdgeKind::Definition },
            );
        }
        
        builder.build()
    }

    /// 型階層を持つグラフ
    pub fn with_type_hierarchy() -> CodeGraph {
        TestGraphBuilder::new()
            .with_typed_symbols(SymbolKind::Class, 3)
            .with_typed_symbols(SymbolKind::Interface, 2)
            .with_edge("Class_0", "Interface_3", EdgeKind::Definition)
            .with_edge("Class_1", "Interface_3", EdgeKind::Definition)
            .with_edge("Class_2", "Interface_4", EdgeKind::Definition)
            .with_edge("Interface_3", "Interface_4", EdgeKind::Definition)
            .build()
    }

    /// 循環参照を持つグラフ
    pub fn with_cycle() -> CodeGraph {
        TestGraphBuilder::new()
            .with_cycle(vec!["cycle_a", "cycle_b", "cycle_c", "cycle_d"])
            .with_symbols(3) // 追加の独立したシンボル
            .build()
    }

    /// 複雑な階層構造
    pub fn complex_hierarchy() -> CodeGraph {
        TestGraphBuilder::new()
            .with_hierarchy("BaseClass", vec!["method1", "method2", "method3"])
            .with_hierarchy("DerivedClass", vec!["method4", "method5"])
            .with_edge("DerivedClass", "BaseClass", EdgeKind::Definition)
            .with_hierarchy("Interface", vec!["interface_method1", "interface_method2"])
            .with_edge("BaseClass", "Interface", EdgeKind::Definition)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let graph = TestGraphBuilder::new()
            .with_symbols(3)
            .build();
        
        assert_eq!(graph.symbol_count(), 3);
    }

    #[test]
    fn test_builder_with_edges() {
        let graph = TestGraphBuilder::new()
            .with_symbols(3)
            .with_edge("symbol_0", "symbol_1", EdgeKind::Reference)
            .build();
        
        assert_eq!(graph.symbol_count(), 3);
        // get_all_symbolsがIteratorを返す
        assert!(graph.get_all_symbols().any(|s| s.id == "symbol_0"));
    }

    #[test]
    fn test_presets() {
        let small = TestGraphPresets::small();
        assert_eq!(small.symbol_count(), 5);
        
        let medium = TestGraphPresets::medium();
        assert_eq!(medium.symbol_count(), 20);
        
        let hierarchy = TestGraphPresets::with_type_hierarchy();
        assert!(hierarchy.symbol_count() > 0);
    }

    #[test]
    fn test_cycle_detection() {
        let graph = TestGraphPresets::with_cycle();
        
        // 循環参照のノードが存在することを確認
        let symbols: Vec<_> = graph.get_all_symbols().collect();
        assert!(symbols.iter().any(|s| s.id == "cycle_a"));
        assert!(symbols.iter().any(|s| s.id == "cycle_b"));
        assert!(symbols.iter().any(|s| s.id == "cycle_c"));
        assert!(symbols.iter().any(|s| s.id == "cycle_d"));
    }
}