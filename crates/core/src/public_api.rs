use crate::graph::{CodeGraph, EdgeKind, Symbol, SymbolKind};

/// 公開API分析のための構造体
#[derive(Debug, Clone)]
pub struct PublicApiAnalyzer {
    graph: CodeGraph,
}

/// APIの可視性レベル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Protected,
    Private,
    Internal,
}

/// API情報
#[derive(Debug, Clone)]
pub struct ApiInfo {
    pub symbol: Symbol,
    pub visibility: Visibility,
    pub is_exported: bool,
    pub reference_count: usize,
    pub importance_score: f64,
}

impl PublicApiAnalyzer {
    pub fn new(graph: CodeGraph) -> Self {
        Self { graph }
    }

    /// 言語固有の公開API判定
    pub fn extract_public_apis(&self, language: &str) -> Vec<ApiInfo> {
        let mut apis = Vec::new();
        
        for node in self.graph.graph.node_indices() {
            if let Some(symbol) = self.graph.graph.node_weight(node) {
                let visibility = self.determine_visibility(symbol, language);
                let is_exported = self.is_exported(node);
                let reference_count = self.count_references(node);
                let importance_score = self.calculate_importance_score(
                    visibility,
                    is_exported,
                    reference_count,
                    symbol.kind,
                );
                
                if visibility == Visibility::Public || is_exported {
                    apis.push(ApiInfo {
                        symbol: symbol.clone(),
                        visibility,
                        is_exported,
                        reference_count,
                        importance_score,
                    });
                }
            }
        }
        
        apis.sort_by(|a, b| b.importance_score.partial_cmp(&a.importance_score).unwrap());
        apis
    }

    /// シンボルの可視性を判定
    fn determine_visibility(&self, symbol: &Symbol, language: &str) -> Visibility {
        match language {
            "rust" => self.determine_rust_visibility(symbol),
            "typescript" | "javascript" => self.determine_typescript_visibility(symbol),
            "python" => self.determine_python_visibility(symbol),
            "go" => self.determine_go_visibility(symbol),
            _ => Visibility::Public, // デフォルトは公開
        }
    }

    /// Rustの可視性判定
    fn determine_rust_visibility(&self, symbol: &Symbol) -> Visibility {
        // nameやdetailからpub修飾子を検出
        if let Some(detail) = &symbol.detail {
            if detail.contains("pub(crate)") {
                return Visibility::Internal;
            }
            if detail.contains("pub") {
                return Visibility::Public;
            }
        }
        
        // デフォルトでプライベート
        Visibility::Private
    }

    /// TypeScript/JavaScriptの可視性判定
    fn determine_typescript_visibility(&self, symbol: &Symbol) -> Visibility {
        if let Some(detail) = &symbol.detail {
            if detail.contains("private") {
                return Visibility::Private;
            }
            if detail.contains("protected") {
                return Visibility::Protected;
            }
            if detail.contains("export") {
                return Visibility::Public;
            }
        }
        
        // exportされていない場合は内部
        Visibility::Internal
    }

    /// Pythonの可視性判定（慣例ベース）
    fn determine_python_visibility(&self, symbol: &Symbol) -> Visibility {
        // マジックメソッド（__init__など）は公開
        if symbol.name.starts_with("__") && symbol.name.ends_with("__") {
            Visibility::Public
        } else if symbol.name.starts_with("__") {
            // プライベート（名前マングリング対象）
            Visibility::Private
        } else if symbol.name.starts_with('_') {
            // プロテクテッド（慣例的な内部使用）
            Visibility::Protected
        } else {
            Visibility::Public
        }
    }

    /// Goの可視性判定（大文字小文字ベース）
    fn determine_go_visibility(&self, symbol: &Symbol) -> Visibility {
        if symbol.name.chars().next().map_or(false, |c| c.is_uppercase()) {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    /// エクスポートされているかチェック
    fn is_exported(&self, node: petgraph::stable_graph::NodeIndex) -> bool {
        // Export エッジを持っているか確認
        self.graph.graph.edges(node)
            .any(|edge| *edge.weight() == EdgeKind::Export)
    }

    /// 参照カウントを取得
    fn count_references(&self, node: petgraph::stable_graph::NodeIndex) -> usize {
        self.graph.graph.edges_directed(node, petgraph::Direction::Incoming)
            .filter(|edge| *edge.weight() == EdgeKind::Reference)
            .count()
    }

    /// 重要度スコアを計算
    fn calculate_importance_score(
        &self,
        visibility: Visibility,
        is_exported: bool,
        reference_count: usize,
        kind: SymbolKind,
    ) -> f64 {
        let mut score = 0.0;
        
        // 可視性によるベーススコア
        score += match visibility {
            Visibility::Public => 1.0,
            Visibility::Protected => 0.5,
            Visibility::Internal => 0.3,
            Visibility::Private => 0.1,
        };
        
        // エクスポートボーナス
        if is_exported {
            score += 2.0;
        }
        
        // 参照数による重み（対数スケール）
        score += (reference_count as f64 + 1.0).ln();
        
        // シンボル種別による重み
        score *= match kind {
            SymbolKind::Module | SymbolKind::Package => 2.0,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Trait => 1.8,
            SymbolKind::Function | SymbolKind::Method => 1.5,
            SymbolKind::Struct | SymbolKind::Enum => 1.4,
            SymbolKind::TypeAlias => 1.2,
            SymbolKind::Variable | SymbolKind::Constant => 1.0,
            _ => 0.8,
        };
        
        score
    }

    /// エントリーポイントを特定
    pub fn identify_entry_points(&self) -> Vec<Symbol> {
        let mut entry_points = Vec::new();
        
        for node in self.graph.graph.node_indices() {
            if let Some(symbol) = self.graph.graph.node_weight(node) {
                if self.is_entry_point(symbol, node) {
                    entry_points.push(symbol.clone());
                }
            }
        }
        
        entry_points
    }

    /// エントリーポイントかどうかを判定
    fn is_entry_point(
        &self,
        symbol: &Symbol,
        node: petgraph::stable_graph::NodeIndex,
    ) -> bool {
        // よくあるエントリーポイント名
        let entry_names = ["main", "index", "app", "server", "cli", "start", "run"];
        if entry_names.contains(&symbol.name.as_str()) {
            return true;
        }
        
        // エクスポートされており、他から参照されていない関数
        if symbol.kind == SymbolKind::Function && self.is_exported(node) {
            let incoming_refs = self.count_references(node);
            if incoming_refs == 0 {
                return true;
            }
        }
        
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range};

    fn create_test_symbol(name: &str, kind: SymbolKind, detail: Option<String>) -> Symbol {
        Symbol {
            id: format!("test_{}", name),
            kind,
            name: name.to_string(),
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            },
            documentation: None,
            detail,
        }
    }

    #[test]
    fn test_rust_visibility() {
        let graph = CodeGraph::new();
        let analyzer = PublicApiAnalyzer::new(graph);

        // Public function
        let pub_fn = create_test_symbol(
            "my_function",
            SymbolKind::Function,
            Some("pub fn my_function()".to_string()),
        );
        assert_eq!(
            analyzer.determine_rust_visibility(&pub_fn),
            Visibility::Public
        );

        // Private function
        let priv_fn = create_test_symbol(
            "internal_function",
            SymbolKind::Function,
            Some("fn internal_function()".to_string()),
        );
        assert_eq!(
            analyzer.determine_rust_visibility(&priv_fn),
            Visibility::Private
        );

        // Crate-public function
        let crate_fn = create_test_symbol(
            "crate_function",
            SymbolKind::Function,
            Some("pub(crate) fn crate_function()".to_string()),
        );
        assert_eq!(
            analyzer.determine_rust_visibility(&crate_fn),
            Visibility::Internal
        );
    }

    #[test]
    fn test_python_visibility() {
        let analyzer = PublicApiAnalyzer::new(CodeGraph::new());

        // Public
        let public = create_test_symbol("public_func", SymbolKind::Function, None);
        assert_eq!(
            analyzer.determine_python_visibility(&public),
            Visibility::Public
        );

        // Protected
        let protected = create_test_symbol("_protected_func", SymbolKind::Function, None);
        assert_eq!(
            analyzer.determine_python_visibility(&protected),
            Visibility::Protected
        );

        // Private
        let private = create_test_symbol("__private_func", SymbolKind::Function, None);
        assert_eq!(
            analyzer.determine_python_visibility(&private),
            Visibility::Private
        );

        // Magic method (should be public)
        let magic = create_test_symbol("__init__", SymbolKind::Method, None);
        assert_eq!(
            analyzer.determine_python_visibility(&magic),
            Visibility::Public
        );
    }

    #[test]
    fn test_go_visibility() {
        let analyzer = PublicApiAnalyzer::new(CodeGraph::new());

        // Public (uppercase)
        let public = create_test_symbol("PublicFunc", SymbolKind::Function, None);
        assert_eq!(
            analyzer.determine_go_visibility(&public),
            Visibility::Public
        );

        // Private (lowercase)
        let private = create_test_symbol("privateFunc", SymbolKind::Function, None);
        assert_eq!(
            analyzer.determine_go_visibility(&private),
            Visibility::Private
        );
    }

    #[test]
    fn test_importance_score() {
        let analyzer = PublicApiAnalyzer::new(CodeGraph::new());

        // Exported module should have high score
        let score = analyzer.calculate_importance_score(
            Visibility::Public,
            true,
            10,
            SymbolKind::Module,
        );
        assert!(score > 5.0);

        // Private variable should have low score
        let score = analyzer.calculate_importance_score(
            Visibility::Private,
            false,
            0,
            SymbolKind::Variable,
        );
        assert!(score < 1.0);
    }

    #[test]
    fn test_entry_point_detection() {
        let mut graph = CodeGraph::new();
        
        // Add main function
        let main_symbol = create_test_symbol("main", SymbolKind::Function, None);
        let main_node = graph.add_symbol(main_symbol.clone());
        
        // Add export edge
        graph.graph.add_edge(main_node, main_node, EdgeKind::Export);
        
        let analyzer = PublicApiAnalyzer::new(graph);
        let entry_points = analyzer.identify_entry_points();
        
        assert_eq!(entry_points.len(), 1);
        assert_eq!(entry_points[0].name, "main");
    }
}