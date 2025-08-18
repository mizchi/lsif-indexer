use petgraph::stable_graph::{StableDiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub kind: SymbolKind,
    pub name: String,
    pub file_path: String,
    pub range: Range,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Parameter,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EdgeKind {
    Definition,
    Reference,
    TypeDefinition,
    Implementation,
    Override,
    Import,
    Export,
    Contains,
}

#[derive(Debug, Clone)]
pub struct CodeGraph {
    pub(crate) graph: StableDiGraph<Symbol, EdgeKind>,
    pub(crate) symbol_index: HashMap<String, NodeIndex>,
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self {
            graph: StableDiGraph::new(),
            symbol_index: HashMap::new(),
        }
    }
}

impl CodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_symbol(&mut self, symbol: Symbol) -> NodeIndex {
        let id = symbol.id.clone();
        let node_index = self.graph.add_node(symbol);
        self.symbol_index.insert(id, node_index);
        node_index
    }

    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, kind: EdgeKind) {
        self.graph.add_edge(from, to, kind);
    }

    pub fn find_symbol(&self, id: &str) -> Option<&Symbol> {
        self.symbol_index.get(id)
            .and_then(|idx| self.graph.node_weight(*idx))
    }

    pub fn find_references(&self, symbol_id: &str) -> Vec<&Symbol> {
        if let Some(&node_idx) = self.symbol_index.get(symbol_id) {
            // このシンボルへの参照（Incoming edges with Reference kind）を探す
            let mut references = Vec::new();
            for edge in self.graph.edges_directed(node_idx, petgraph::Direction::Incoming) {
                if matches!(edge.weight(), EdgeKind::Reference) {
                    if let Some(symbol) = self.graph.node_weight(edge.source()) {
                        references.push(symbol);
                    }
                }
            }
            references
        } else {
            Vec::new()
        }
    }

    pub fn find_definition(&self, reference_id: &str) -> Option<&Symbol> {
        if let Some(&node_idx) = self.symbol_index.get(reference_id) {
            for edge in self.graph.edges_directed(node_idx, petgraph::Direction::Incoming) {
                if matches!(edge.weight(), EdgeKind::Definition) {
                    return self.graph.node_weight(edge.source());
                }
            }
        }
        None
    }

    pub fn symbol_count(&self) -> usize {
        self.symbol_index.len()
    }

    pub fn get_node_index(&self, symbol_id: &str) -> Option<NodeIndex> {
        self.symbol_index.get(symbol_id).copied()
    }
    
    pub fn get_all_symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.graph.node_weights()
    }

    pub fn find_definition_at(&self, file_path: &str, position: Position) -> Option<&Symbol> {
        // 指定された位置にあるシンボルを探す
        for symbol in self.graph.node_weights() {
            if symbol.file_path == file_path 
                && symbol.range.start.line <= position.line 
                && symbol.range.end.line >= position.line
                && symbol.range.start.character <= position.character
                && symbol.range.end.character >= position.character {
                
                // このシンボルが参照している定義を探す
                if let Some(&node_idx) = self.symbol_index.get(&symbol.id) {
                    for edge in self.graph.edges(node_idx) {
                        if matches!(edge.weight(), EdgeKind::Reference) {
                            return self.graph.node_weight(edge.target());
                        }
                    }
                }
                // シンボル自体が定義の場合
                return Some(symbol);
            }
        }
        None
    }

    pub fn find_implementations(&self, interface_id: &str) -> Vec<&Symbol> {
        if let Some(&node_idx) = self.symbol_index.get(interface_id) {
            // インターフェースを実装しているクラスを探す
            let mut implementations = Vec::new();
            for edge in self.graph.edges_directed(node_idx, petgraph::Direction::Incoming) {
                if matches!(edge.weight(), EdgeKind::Implementation) {
                    if let Some(symbol) = self.graph.node_weight(edge.source()) {
                        implementations.push(symbol);
                    }
                }
            }
            implementations
        } else {
            Vec::new()
        }
    }

    pub fn find_overrides(&self, method_id: &str) -> Vec<&Symbol> {
        if let Some(&node_idx) = self.symbol_index.get(method_id) {
            // このメソッドをオーバーライドしているメソッドを探す
            let mut overrides = Vec::new();
            for edge in self.graph.edges_directed(node_idx, petgraph::Direction::Incoming) {
                if matches!(edge.weight(), EdgeKind::Override) {
                    if let Some(symbol) = self.graph.node_weight(edge.source()) {
                        overrides.push(symbol);
                    }
                }
            }
            overrides
        } else {
            Vec::new()
        }
    }
}