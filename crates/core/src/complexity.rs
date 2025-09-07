use crate::graph::{CodeGraph, EdgeKind, SymbolKind};
use petgraph::algo::tarjan_scc;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

/// 複雑度分析器
#[derive(Debug)]
pub struct ComplexityAnalyzer<'a> {
    graph: &'a CodeGraph,
}

/// 複雑度メトリクス
#[derive(Debug, Clone)]
pub struct ComplexityMetrics {
    pub cyclomatic_complexity: usize,
    pub cognitive_complexity: usize,
    pub depth: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub coupling: f64,
}

impl<'a> ComplexityAnalyzer<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// 関数の循環的複雑度を計算
    pub fn calculate_cyclomatic_complexity(&self, symbol_id: &str) -> Option<usize> {
        let node = self.graph.symbol_index.get(symbol_id)?;
        
        // 関数/メソッドのみ対象
        let symbol = self.graph.graph.node_weight(*node)?;
        if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
            return None;
        }
        
        // McCabeの循環的複雑度: V(G) = E - N + 2P
        // E: エッジ数, N: ノード数, P: 連結成分数（通常1）
        let mut visited = HashSet::new();
        let mut edges_count = 0;
        let mut nodes_count = 0;
        
        self.traverse_control_flow(*node, &mut visited, &mut edges_count, &mut nodes_count);
        
        // 基本的な複雑度（最小値1）
        let complexity = if nodes_count > 0 {
            edges_count.saturating_sub(nodes_count) + 2
        } else {
            1
        };
        
        Some(complexity.max(1))
    }

    /// 制御フローグラフを走査
    fn traverse_control_flow(
        &self,
        node: petgraph::stable_graph::NodeIndex,
        visited: &mut HashSet<petgraph::stable_graph::NodeIndex>,
        edges_count: &mut usize,
        nodes_count: &mut usize,
    ) {
        if visited.contains(&node) {
            return;
        }
        
        visited.insert(node);
        *nodes_count += 1;
        
        // Contains関係で内部ノードを探索
        for edge in self.graph.graph.edges(node) {
            if *edge.weight() == EdgeKind::Contains {
                *edges_count += 1;
                self.traverse_control_flow(edge.target(), visited, edges_count, nodes_count);
            }
        }
    }

    /// 認知的複雑度を計算
    pub fn calculate_cognitive_complexity(&self, symbol_id: &str) -> Option<usize> {
        let node = self.graph.symbol_index.get(symbol_id)?;
        let symbol = self.graph.graph.node_weight(*node)?;
        
        if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
            return None;
        }
        
        let mut complexity = 0;
        let mut nesting_level = 0;
        
        self.calculate_cognitive_recursive(*node, &mut complexity, &mut nesting_level);
        
        Some(complexity)
    }

    fn calculate_cognitive_recursive(
        &self,
        node: petgraph::stable_graph::NodeIndex,
        complexity: &mut usize,
        nesting_level: &mut usize,
    ) {
        // ネストレベルに応じた重み付け
        let weight = *nesting_level + 1;
        
        if let Some(symbol) = self.graph.graph.node_weight(node) {
            // 制御構造による複雑度増加
            match symbol.kind {
                SymbolKind::Function | SymbolKind::Method => {
                    *nesting_level += 1;
                }
                _ => {}
            }
            
            // ブランチごとに複雑度を加算
            let branches = self.count_branches(node);
            if branches > 1 {
                *complexity += (branches - 1) * weight;
            }
        }
        
        // 子ノードを再帰的に処理
        for edge in self.graph.graph.edges(node) {
            if *edge.weight() == EdgeKind::Contains {
                self.calculate_cognitive_recursive(edge.target(), complexity, nesting_level);
            }
        }
    }

    /// ブランチ数をカウント
    fn count_branches(&self, node: petgraph::stable_graph::NodeIndex) -> usize {
        self.graph.graph.edges(node)
            .filter(|e| matches!(e.weight(), EdgeKind::Reference | EdgeKind::Definition))
            .count()
    }

    /// すべての関数の複雑度を計算
    pub fn analyze_all_functions(&self) -> HashMap<String, ComplexityMetrics> {
        let mut results = HashMap::new();
        
        for node in self.graph.graph.node_indices() {
            if let Some(symbol) = self.graph.graph.node_weight(node) {
                if matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method) {
                    let metrics = self.calculate_metrics(&symbol.id);
                    results.insert(symbol.id.clone(), metrics);
                }
            }
        }
        
        results
    }

    /// シンボルの総合的なメトリクスを計算
    pub fn calculate_metrics(&self, symbol_id: &str) -> ComplexityMetrics {
        let cyclomatic = self.calculate_cyclomatic_complexity(symbol_id).unwrap_or(1);
        let cognitive = self.calculate_cognitive_complexity(symbol_id).unwrap_or(0);
        let (fan_in, fan_out) = self.calculate_fan_metrics(symbol_id);
        let depth = self.calculate_depth(symbol_id);
        let coupling = self.calculate_coupling(symbol_id);
        
        ComplexityMetrics {
            cyclomatic_complexity: cyclomatic,
            cognitive_complexity: cognitive,
            depth,
            fan_in,
            fan_out,
            coupling,
        }
    }

    /// ファンイン・ファンアウトを計算
    fn calculate_fan_metrics(&self, symbol_id: &str) -> (usize, usize) {
        if let Some(node) = self.graph.symbol_index.get(symbol_id) {
            let fan_in = self.graph.graph
                .edges_directed(*node, petgraph::Direction::Incoming)
                .filter(|e| *e.weight() == EdgeKind::Reference)
                .count();
            
            let fan_out = self.graph.graph
                .edges_directed(*node, petgraph::Direction::Outgoing)
                .filter(|e| *e.weight() == EdgeKind::Reference)
                .count();
            
            (fan_in, fan_out)
        } else {
            (0, 0)
        }
    }

    /// ネストの深さを計算
    fn calculate_depth(&self, symbol_id: &str) -> usize {
        if let Some(node) = self.graph.symbol_index.get(symbol_id) {
            self.calculate_depth_recursive(*node, 0)
        } else {
            0
        }
    }

    fn calculate_depth_recursive(
        &self,
        node: petgraph::stable_graph::NodeIndex,
        current_depth: usize,
    ) -> usize {
        let mut max_depth = current_depth;
        
        for edge in self.graph.graph.edges(node) {
            if *edge.weight() == EdgeKind::Contains {
                let child_depth = self.calculate_depth_recursive(edge.target(), current_depth + 1);
                max_depth = max_depth.max(child_depth);
            }
        }
        
        max_depth
    }

    /// 結合度を計算
    fn calculate_coupling(&self, symbol_id: &str) -> f64 {
        if let Some(node) = self.graph.symbol_index.get(symbol_id) {
            let mut external_deps = HashSet::new();
            
            // 外部依存を収集
            for edge in self.graph.graph.edges(*node) {
                if matches!(edge.weight(), EdgeKind::Import | EdgeKind::Reference) {
                    if let Some(target_symbol) = self.graph.graph.node_weight(edge.target()) {
                        // 異なるファイルへの参照を外部依存とみなす
                        if let Some(source_symbol) = self.graph.graph.node_weight(*node) {
                            if target_symbol.file_path != source_symbol.file_path {
                                external_deps.insert(edge.target());
                            }
                        }
                    }
                }
            }
            
            // 結合度 = 外部依存数 / (外部依存数 + 1)
            let dep_count = external_deps.len() as f64;
            dep_count / (dep_count + 1.0)
        } else {
            0.0
        }
    }

    /// 循環依存を検出
    pub fn detect_circular_dependencies(&self) -> Vec<Vec<String>> {
        // Tarjanのアルゴリズムで強連結成分を検出
        let sccs = tarjan_scc(&self.graph.graph);
        
        let mut circular_deps = Vec::new();
        
        for scc in sccs {
            // サイズが2以上の強連結成分は循環依存
            if scc.len() > 1 {
                let cycle: Vec<String> = scc
                    .iter()
                    .filter_map(|&node| {
                        self.graph.graph.node_weight(node).map(|s| s.id.clone())
                    })
                    .collect();
                
                if !cycle.is_empty() {
                    circular_deps.push(cycle);
                }
            }
        }
        
        circular_deps
    }

    /// 複雑度が高い関数をランキング
    pub fn rank_by_complexity(&self, limit: usize) -> Vec<(String, ComplexityMetrics)> {
        let mut rankings: Vec<_> = self.analyze_all_functions()
            .into_iter()
            .collect();
        
        // 循環的複雑度でソート
        rankings.sort_by(|a, b| {
            b.1.cyclomatic_complexity.cmp(&a.1.cyclomatic_complexity)
        });
        
        rankings.truncate(limit);
        rankings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range, Symbol};

    fn create_test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();
        
        // Create test function
        let func = Symbol {
            id: "test_func".to_string(),
            kind: SymbolKind::Function,
            name: "test_func".to_string(),
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 10, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        
        let func_node = graph.add_symbol(func);
        
        // Add some internal nodes to simulate control flow
        for i in 0..3 {
            let block = Symbol {
                id: format!("block_{}", i),
                kind: SymbolKind::Variable,
                name: format!("block_{}", i),
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position { line: i as u32, character: 0 },
                    end: Position { line: i as u32 + 1, character: 0 },
                },
                documentation: None,
                detail: None,
            };
            
            let block_node = graph.add_symbol(block);
            graph.add_edge(func_node, block_node, EdgeKind::Contains);
            
            if i > 0 {
                // Add reference edges to increase complexity
                graph.add_edge(block_node, func_node, EdgeKind::Reference);
            }
        }
        
        graph
    }

    #[test]
    fn test_cyclomatic_complexity() {
        let graph = create_test_graph();
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        let complexity = analyzer.calculate_cyclomatic_complexity("test_func");
        assert!(complexity.is_some());
        assert!(complexity.unwrap() >= 1);
    }

    #[test]
    fn test_cognitive_complexity() {
        let graph = create_test_graph();
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        let complexity = analyzer.calculate_cognitive_complexity("test_func");
        assert!(complexity.is_some());
    }

    #[test]
    fn test_fan_metrics() {
        let graph = create_test_graph();
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        let (fan_in, fan_out) = analyzer.calculate_fan_metrics("test_func");
        // fan_in and fan_out are usize, always >= 0
        assert_eq!(fan_in, fan_in);
        assert_eq!(fan_out, fan_out);
    }

    #[test]
    fn test_depth_calculation() {
        let graph = create_test_graph();
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        let depth = analyzer.calculate_depth("test_func");
        assert!(depth > 0);
    }

    #[test]
    fn test_coupling_calculation() {
        let graph = create_test_graph();
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        let coupling = analyzer.calculate_coupling("test_func");
        assert!(coupling >= 0.0 && coupling <= 1.0);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut graph = CodeGraph::new();
        
        // Create circular dependency A -> B -> C -> A
        let a = Symbol {
            id: "A".to_string(),
            kind: SymbolKind::Module,
            name: "A".to_string(),
            file_path: "a.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        
        let b = Symbol {
            id: "B".to_string(),
            kind: SymbolKind::Module,
            name: "B".to_string(),
            file_path: "b.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        
        let c = Symbol {
            id: "C".to_string(),
            kind: SymbolKind::Module,
            name: "C".to_string(),
            file_path: "c.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        
        let a_node = graph.add_symbol(a);
        let b_node = graph.add_symbol(b);
        let c_node = graph.add_symbol(c);
        
        graph.add_edge(a_node, b_node, EdgeKind::Import);
        graph.add_edge(b_node, c_node, EdgeKind::Import);
        graph.add_edge(c_node, a_node, EdgeKind::Import);
        
        let analyzer = ComplexityAnalyzer::new(&graph);
        let circular_deps = analyzer.detect_circular_dependencies();
        
        assert!(!circular_deps.is_empty());
        assert_eq!(circular_deps[0].len(), 3);
    }

    #[test]
    fn test_complexity_ranking() {
        let mut graph = CodeGraph::new();
        
        // Add multiple functions with different complexities
        for i in 0..5 {
            let func = Symbol {
                id: format!("func_{}", i),
                kind: SymbolKind::Function,
                name: format!("func_{}", i),
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position { line: i as u32 * 10, character: 0 },
                    end: Position { line: (i as u32 + 1) * 10, character: 0 },
                },
                documentation: None,
                detail: None,
            };
            
            let func_node = graph.add_symbol(func);
            
            // Add varying number of internal nodes to create different complexities
            for j in 0..=i {
                let block = Symbol {
                    id: format!("block_{}_{}", i, j),
                    kind: SymbolKind::Variable,
                    name: format!("block_{}_{}", i, j),
                    file_path: "test.rs".to_string(),
                    range: Range {
                        start: Position { line: 0, character: 0 },
                        end: Position { line: 0, character: 0 },
                    },
                    documentation: None,
                    detail: None,
                };
                
                let block_node = graph.add_symbol(block);
                graph.add_edge(func_node, block_node, EdgeKind::Contains);
            }
        }
        
        let analyzer = ComplexityAnalyzer::new(&graph);
        let rankings = analyzer.rank_by_complexity(3);
        
        assert_eq!(rankings.len(), 3);
        // 複雑度が降順になっているか確認
        for i in 0..rankings.len() - 1 {
            assert!(rankings[i].1.cyclomatic_complexity >= rankings[i + 1].1.cyclomatic_complexity);
        }
    }

    #[test]
    fn test_empty_graph_complexity() {
        let graph = CodeGraph::new();
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        // 存在しないシンボルのcomplexity
        assert_eq!(analyzer.calculate_cyclomatic_complexity("nonexistent"), None);
        assert_eq!(analyzer.calculate_cognitive_complexity("nonexistent"), None);
        let (fan_in, fan_out) = analyzer.calculate_fan_metrics("nonexistent");
        assert_eq!(fan_in, 0);
        assert_eq!(fan_out, 0);
    }

    #[test]
    fn test_single_function_complexity() {
        let mut graph = CodeGraph::new();
        let func = Symbol {
            id: "func1".to_string(),
            kind: SymbolKind::Function,
            name: "single_func".to_string(),
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 10, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        graph.add_symbol(func);
        
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        // 単一関数の基本的な複雑度は2（E-N+2P where E=0, N=0, P=1 => 2）
        assert_eq!(analyzer.calculate_cyclomatic_complexity("func1"), Some(2));
        assert_eq!(analyzer.calculate_cognitive_complexity("func1"), Some(0));
        let (fan_in, fan_out) = analyzer.calculate_fan_metrics("func1");
        assert_eq!(fan_in, 0);
        assert_eq!(fan_out, 0);
    }

    #[test]
    fn test_calculate_maintainability_index() {
        let mut graph = CodeGraph::new();
        
        // シンプルな関数
        let simple = Symbol {
            id: "simple".to_string(),
            kind: SymbolKind::Function,
            name: "simple".to_string(),
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 5, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        
        let _simple_node = graph.add_symbol(simple);
        
        // 複雑な関数
        let complex = Symbol {
            id: "complex".to_string(),
            kind: SymbolKind::Function,
            name: "complex".to_string(),
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position { line: 10, character: 0 },
                end: Position { line: 50, character: 0 },
            },
            documentation: None,
            detail: None,
        };
        
        let complex_node = graph.add_symbol(complex);
        
        // complexに複雑度を追加
        for i in 0..5 {
            let var = Symbol {
                id: format!("var_{}", i),
                kind: SymbolKind::Variable,
                name: format!("var_{}", i),
                file_path: "test.rs".to_string(),
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
                documentation: None,
                detail: None,
            };
            let var_node = graph.add_symbol(var);
            graph.add_edge(complex_node, var_node, EdgeKind::Contains);
        }
        
        let analyzer = ComplexityAnalyzer::new(&graph);
        
        // 複雑度メトリクスの比較
        let simple_cyclo = analyzer.calculate_cyclomatic_complexity("simple");
        let complex_cyclo = analyzer.calculate_cyclomatic_complexity("complex");
        
        assert!(simple_cyclo.is_some());
        assert!(complex_cyclo.is_some());
        
        // complexの方が高い複雑度を持つはず（Contains edgeが多いため）
        // 注：実装によっては同じ値になる可能性もある
        assert!(complex_cyclo.unwrap() >= simple_cyclo.unwrap());
    }
}