use super::graph::{CodeGraph, Symbol, EdgeKind};

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
            if let (Some(&from_idx), Some(&to_idx)) = 
                (self.graph.symbol_index.get(&from_id), self.graph.symbol_index.get(&to_id)) {
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