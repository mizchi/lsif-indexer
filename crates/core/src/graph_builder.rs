use super::graph::{CodeGraph, EdgeKind, Symbol};

/// グラフ構築を効率化するビルダーパターン
pub struct GraphBuilder {
    graph: CodeGraph,
    pending_edges: Vec<(String, String, EdgeKind)>,
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::new(),
            pending_edges: Vec::new(),
        }
    }

    /// シンボルを追加（IDの重複チェック付き）
    pub fn add_symbol(&mut self, symbol: Symbol) -> &mut Self {
        if !self.graph.symbol_index.contains_key(&symbol.id) {
            self.graph.add_symbol(symbol);
        }
        self
    }

    /// 遅延エッジ追加（シンボルが全て追加された後に解決）
    pub fn add_edge_by_id(&mut self, from_id: String, to_id: String, kind: EdgeKind) -> &mut Self {
        self.pending_edges.push((from_id, to_id, kind));
        self
    }

    /// 保留中のエッジを解決してグラフを構築
    pub fn build(mut self) -> CodeGraph {
        for (from_id, to_id, kind) in self.pending_edges {
            if let (Some(&from_idx), Some(&to_idx)) = (
                self.graph.symbol_index.get(&from_id),
                self.graph.symbol_index.get(&to_id),
            ) {
                self.graph.add_edge(from_idx, to_idx, kind);
            }
        }
        self.graph
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, SymbolKind};

    fn create_test_symbol(id: &str, name: &str) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            documentation: None,
            detail: None,
        }
    }

    #[test]
    fn test_new_builder() {
        let builder = GraphBuilder::new();
        assert_eq!(builder.pending_edges.len(), 0);
        assert_eq!(builder.graph.symbol_count(), 0);
    }

    #[test]
    fn test_default_builder() {
        let builder = GraphBuilder::default();
        assert_eq!(builder.pending_edges.len(), 0);
        assert_eq!(builder.graph.symbol_count(), 0);
    }

    #[test]
    fn test_add_symbol() {
        let mut builder = GraphBuilder::new();
        let symbol = create_test_symbol("test1", "TestSymbol");

        builder.add_symbol(symbol.clone());
        assert_eq!(builder.graph.symbol_count(), 1);

        // 同じIDのシンボルを追加しても重複しない
        builder.add_symbol(symbol);
        assert_eq!(builder.graph.symbol_count(), 1);
    }

    #[test]
    fn test_add_edge_by_id() {
        let mut builder = GraphBuilder::new();

        builder.add_edge_by_id("from".to_string(), "to".to_string(), EdgeKind::Reference);
        assert_eq!(builder.pending_edges.len(), 1);

        builder.add_edge_by_id("from2".to_string(), "to2".to_string(), EdgeKind::Definition);
        assert_eq!(builder.pending_edges.len(), 2);
    }

    #[test]
    fn test_build_with_edges() {
        let mut builder = GraphBuilder::new();

        // シンボルを追加
        let symbol1 = create_test_symbol("sym1", "Symbol1");
        let symbol2 = create_test_symbol("sym2", "Symbol2");
        let symbol3 = create_test_symbol("sym3", "Symbol3");

        builder
            .add_symbol(symbol1)
            .add_symbol(symbol2)
            .add_symbol(symbol3);

        // エッジを追加
        builder
            .add_edge_by_id("sym1".to_string(), "sym2".to_string(), EdgeKind::Reference)
            .add_edge_by_id("sym2".to_string(), "sym3".to_string(), EdgeKind::Definition);

        // グラフを構築
        let graph = builder.build();
        assert_eq!(graph.symbol_count(), 3);

        // エッジが正しく追加されているか確認
        let references = graph.find_references("sym2").unwrap();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].id, "sym1");
    }

    #[test]
    fn test_build_with_invalid_edges() {
        let mut builder = GraphBuilder::new();

        // シンボルを追加
        let symbol1 = create_test_symbol("sym1", "Symbol1");
        builder.add_symbol(symbol1);

        // 存在しないシンボルへのエッジを追加
        builder
            .add_edge_by_id(
                "sym1".to_string(),
                "nonexistent".to_string(),
                EdgeKind::Reference,
            )
            .add_edge_by_id(
                "nonexistent".to_string(),
                "sym1".to_string(),
                EdgeKind::Reference,
            );

        // グラフを構築（無効なエッジは無視される）
        let graph = builder.build();
        assert_eq!(graph.symbol_count(), 1);

        // 無効なエッジは追加されていない
        let references = graph.find_references("sym1").unwrap();
        assert_eq!(references.len(), 0);
    }

    #[test]
    fn test_method_chaining() {
        let mut builder = GraphBuilder::new();
        builder
            .add_symbol(create_test_symbol("a", "A"))
            .add_symbol(create_test_symbol("b", "B"))
            .add_edge_by_id("a".to_string(), "b".to_string(), EdgeKind::Reference);

        let graph = builder.build();
        assert_eq!(graph.symbol_count(), 2);
    }
}
