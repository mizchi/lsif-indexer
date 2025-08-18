use std::collections::{HashMap, HashSet, VecDeque};
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use super::{Symbol, SymbolKind, EdgeKind, Range};

/// 拡張されたコードグラフ（高度な解析機能付き）
#[derive(Debug, Clone)]
pub struct EnhancedCodeGraph {
    pub graph: StableDiGraph<Symbol, EdgeKind>,
    pub symbol_index: HashMap<String, NodeIndex>,
    /// ファイルごとのシンボルインデックス
    pub file_symbols: HashMap<String, Vec<NodeIndex>>,
    /// 型の継承関係
    pub type_hierarchy: HashMap<String, TypeInfo>,
    /// 関数呼び出し関係
    pub call_graph: HashMap<String, CallInfo>,
    /// シンボルの使用回数
    pub usage_count: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    pub symbol_id: String,
    pub base_types: Vec<String>,      // 継承元
    pub derived_types: Vec<String>,    // 派生型
    pub implements: Vec<String>,       // 実装インターフェース
    pub methods: Vec<String>,          // メソッド一覧
    pub fields: Vec<String>,           // フィールド一覧
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallInfo {
    pub symbol_id: String,
    pub calls: Vec<String>,           // この関数が呼び出す関数
    pub called_by: Vec<String>,       // この関数を呼び出す関数
    pub depth: usize,                 // コールスタックの深さ
}

impl EnhancedCodeGraph {
    pub fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            symbol_index: HashMap::new(),
            file_symbols: HashMap::new(),
            type_hierarchy: HashMap::new(),
            call_graph: HashMap::new(),
            usage_count: HashMap::new(),
        }
    }

    /// 改善されたシンボル追加（統一ID形式）
    pub fn add_symbol_enhanced(&mut self, mut symbol: Symbol) -> NodeIndex {
        // ID形式を統一: file_path#line:column:name
        if !symbol.id.contains("#") {
            symbol.id = format!(
                "{}#{}:{}:{}",
                symbol.file_path,
                symbol.range.start.line,
                symbol.range.start.character,
                symbol.name
            );
        }

        let node_index = self.graph.add_node(symbol.clone());
        self.symbol_index.insert(symbol.id.clone(), node_index);
        
        // ファイルインデックスに追加
        self.file_symbols
            .entry(symbol.file_path.clone())
            .or_insert_with(Vec::new)
            .push(node_index);
        
        // 使用回数を初期化
        self.usage_count.insert(symbol.id.clone(), 0);
        
        node_index
    }

    /// エッジ追加時に関連情報を更新
    pub fn add_edge_enhanced(&mut self, from: NodeIndex, to: NodeIndex, kind: EdgeKind) {
        self.graph.add_edge(from, to, kind.clone());
        
        // 使用回数を更新
        if let Some(to_symbol) = self.graph.node_weight(to) {
            *self.usage_count.entry(to_symbol.id.clone()).or_insert(0) += 1;
        }
        
        // コールグラフを更新
        if matches!(kind, EdgeKind::Reference) {
            if let (Some(from_symbol), Some(to_symbol)) = 
                (self.graph.node_weight(from), self.graph.node_weight(to)) {
                
                if matches!(from_symbol.kind, SymbolKind::Function | SymbolKind::Method) {
                    let from_info = self.call_graph
                        .entry(from_symbol.id.clone())
                        .or_insert_with(|| CallInfo {
                            symbol_id: from_symbol.id.clone(),
                            calls: Vec::new(),
                            called_by: Vec::new(),
                            depth: 0,
                        });
                    from_info.calls.push(to_symbol.id.clone());
                    
                    let to_info = self.call_graph
                        .entry(to_symbol.id.clone())
                        .or_insert_with(|| CallInfo {
                            symbol_id: to_symbol.id.clone(),
                            calls: Vec::new(),
                            called_by: Vec::new(),
                            depth: 0,
                        });
                    to_info.called_by.push(from_symbol.id.clone());
                }
            }
        }
        
        // 型階層を更新
        if matches!(kind, EdgeKind::TypeDefinition | EdgeKind::Implementation) {
            if let (Some(from_symbol), Some(to_symbol)) = 
                (self.graph.node_weight(from), self.graph.node_weight(to)) {
                
                let from_info = self.type_hierarchy
                    .entry(from_symbol.id.clone())
                    .or_insert_with(|| TypeInfo {
                        symbol_id: from_symbol.id.clone(),
                        base_types: Vec::new(),
                        derived_types: Vec::new(),
                        implements: Vec::new(),
                        methods: Vec::new(),
                        fields: Vec::new(),
                    });
                
                match kind {
                    EdgeKind::TypeDefinition => {
                        from_info.base_types.push(to_symbol.id.clone());
                        
                        let to_info = self.type_hierarchy
                            .entry(to_symbol.id.clone())
                            .or_insert_with(|| TypeInfo {
                                symbol_id: to_symbol.id.clone(),
                                base_types: Vec::new(),
                                derived_types: Vec::new(),
                                implements: Vec::new(),
                                methods: Vec::new(),
                                fields: Vec::new(),
                            });
                        to_info.derived_types.push(from_symbol.id.clone());
                    }
                    EdgeKind::Implementation => {
                        from_info.implements.push(to_symbol.id.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    /// 改善された定義検索
    pub fn find_definition_enhanced(&self, reference_pos: &str) -> Option<&Symbol> {
        // reference_pos形式: file_path#line:column
        let parts: Vec<&str> = reference_pos.split('#').collect();
        if parts.len() != 2 {
            return None;
        }
        
        let file_path = parts[0];
        let pos_parts: Vec<&str> = parts[1].split(':').collect();
        if pos_parts.len() < 2 {
            return None;
        }
        
        let line: u32 = pos_parts[0].parse().ok()?;
        let column: u32 = pos_parts[1].parse().ok()?;
        
        // ファイル内のシンボルを検索
        if let Some(file_symbols) = self.file_symbols.get(file_path) {
            for &node_idx in file_symbols {
                if let Some(symbol) = self.graph.node_weight(node_idx) {
                    // 位置がシンボルの範囲内にあるか確認
                    if symbol.range.contains(line, column) {
                        // このシンボルの定義を探す
                        for edge in self.graph.edges_directed(node_idx, Direction::Outgoing) {
                            if matches!(edge.weight(), EdgeKind::Definition) {
                                return self.graph.node_weight(edge.target());
                            }
                        }
                        // シンボル自体が定義の場合
                        return Some(symbol);
                    }
                }
            }
        }
        
        None
    }

    /// 改善された参照検索
    pub fn find_references_enhanced(&self, symbol_id: &str) -> Vec<&Symbol> {
        let mut references = Vec::new();
        
        if let Some(&node_idx) = self.symbol_index.get(symbol_id) {
            // 入力エッジ（このシンボルを参照している）を探す
            for edge in self.graph.edges_directed(node_idx, Direction::Incoming) {
                if matches!(edge.weight(), EdgeKind::Reference) {
                    if let Some(symbol) = self.graph.node_weight(edge.source()) {
                        references.push(symbol);
                    }
                }
            }
        }
        
        references
    }

    /// コールグラフの取得
    pub fn get_call_hierarchy(&self, function_id: &str, max_depth: usize) -> CallHierarchy {
        let mut hierarchy = CallHierarchy {
            root: function_id.to_string(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
        };
        
        // BFSで呼び出し階層を探索
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((function_id.to_string(), 0));
        
        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth || !visited.insert(current_id.clone()) {
                continue;
            }
            
            if let Some(info) = self.call_graph.get(&current_id) {
                // 呼び出し先
                for called in &info.calls {
                    hierarchy.outgoing.push(CallNode {
                        symbol_id: called.clone(),
                        depth: depth + 1,
                        symbol: self.get_symbol(called).map(|s| s.name.clone()),
                    });
                    queue.push_back((called.clone(), depth + 1));
                }
                
                // 呼び出し元
                for caller in &info.called_by {
                    hierarchy.incoming.push(CallNode {
                        symbol_id: caller.clone(),
                        depth: depth + 1,
                        symbol: self.get_symbol(caller).map(|s| s.name.clone()),
                    });
                }
            }
        }
        
        hierarchy
    }

    /// デッドコード検出
    pub fn find_dead_code(&self) -> Vec<&Symbol> {
        let mut dead_symbols = Vec::new();
        
        for (symbol_id, &count) in &self.usage_count {
            if count == 0 {
                if let Some(symbol) = self.get_symbol(symbol_id) {
                    // エントリーポイント（main関数など）は除外
                    if !self.is_entry_point(symbol) {
                        dead_symbols.push(symbol);
                    }
                }
            }
        }
        
        dead_symbols
    }

    /// 型関係の解析
    pub fn analyze_type_relations(&self, type_id: &str) -> TypeRelations {
        let mut relations = TypeRelations {
            type_id: type_id.to_string(),
            base_types: Vec::new(),
            derived_types: Vec::new(),
            implementations: Vec::new(),
            methods: Vec::new(),
            fields: Vec::new(),
            related_types: HashSet::new(),
        };
        
        if let Some(info) = self.type_hierarchy.get(type_id) {
            relations.base_types = info.base_types.clone();
            relations.derived_types = info.derived_types.clone();
            relations.implementations = info.implements.clone();
            relations.methods = info.methods.clone();
            relations.fields = info.fields.clone();
            
            // 関連型を収集
            for base in &info.base_types {
                relations.related_types.insert(base.clone());
            }
            for derived in &info.derived_types {
                relations.related_types.insert(derived.clone());
            }
            for impl_type in &info.implements {
                relations.related_types.insert(impl_type.clone());
            }
        }
        
        relations
    }

    /// クロスファイル解析
    pub fn analyze_cross_file_dependencies(&self) -> HashMap<String, Vec<String>> {
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
        
        for edge in self.graph.edge_indices() {
            if let Some((source, target)) = self.graph.edge_endpoints(edge) {
                if let (Some(from_symbol), Some(to_symbol)) = 
                    (self.graph.node_weight(source), 
                     self.graph.node_weight(target)) {
                
                if from_symbol.file_path != to_symbol.file_path {
                    dependencies
                        .entry(from_symbol.file_path.clone())
                        .or_insert_with(Vec::new)
                        .push(to_symbol.file_path.clone());
                }
            }
        }
    }
        
        // 重複を削除
        for deps in dependencies.values_mut() {
            deps.sort();
            deps.dedup();
        }
        
        dependencies
    }

    // ヘルパーメソッド
    fn get_symbol(&self, symbol_id: &str) -> Option<&Symbol> {
        self.symbol_index.get(symbol_id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    fn is_entry_point(&self, symbol: &Symbol) -> bool {
        symbol.name == "main" || 
        symbol.name == "Main" ||
        symbol.name.ends_with("::main") ||
        symbol.name.contains("test") ||
        symbol.name.starts_with("test_")
    }

    pub fn symbol_count(&self) -> usize {
        self.symbol_index.len()
    }
}

impl Range {
    /// 指定された位置が範囲内にあるか確認
    pub fn contains(&self, line: u32, character: u32) -> bool {
        if line < self.start.line || line > self.end.line {
            return false;
        }
        
        if line == self.start.line && character < self.start.character {
            return false;
        }
        
        if line == self.end.line && character > self.end.character {
            return false;
        }
        
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchy {
    pub root: String,
    pub outgoing: Vec<CallNode>,
    pub incoming: Vec<CallNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallNode {
    pub symbol_id: String,
    pub depth: usize,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeRelations {
    pub type_id: String,
    pub base_types: Vec<String>,
    pub derived_types: Vec<String>,
    pub implementations: Vec<String>,
    pub methods: Vec<String>,
    pub fields: Vec<String>,
    pub related_types: HashSet<String>,
}

impl Default for EnhancedCodeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced index structure for LSP integration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnhancedIndex {
    pub symbols: HashMap<String, Symbol>,
    pub references: HashMap<String, Vec<Reference>>,
    pub definitions: HashMap<String, Vec<String>>,
    pub call_graph: Vec<CallEdge>,
    pub type_relations: Vec<TypeRelations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub location: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdge {
    pub from: String,
    pub to: String,
    pub call_site: String,
}