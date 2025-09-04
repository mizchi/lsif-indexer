use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Symbol {
    pub id: String,
    pub kind: SymbolKind,
    pub name: String,
    pub file_path: String,
    pub range: Range,
    pub documentation: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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
    Reference,
    Trait,
    TypeAlias,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    pub graph: StableDiGraph<Symbol, EdgeKind>,
    pub symbol_index: HashMap<String, NodeIndex>,
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

    /// バルク挿入用：複数のシンボルを効率的に追加
    pub fn add_symbols(&mut self, symbols: Vec<Symbol>) {
        // 事前にインデックスのキャパシティを確保
        self.symbol_index.reserve(symbols.len());

        for symbol in symbols {
            let id = symbol.id.clone();
            let node_index = self.graph.add_node(symbol);
            self.symbol_index.insert(id, node_index);
        }
    }

    pub fn remove_symbol(&mut self, id: &str) -> bool {
        if let Some(node_index) = self.symbol_index.remove(id) {
            self.graph.remove_node(node_index);
            true
        } else {
            false
        }
    }

    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, kind: EdgeKind) {
        self.graph.add_edge(from, to, kind);
    }

    pub fn find_symbol(&self, id: &str) -> Option<&Symbol> {
        self.symbol_index
            .get(id)
            .and_then(|idx| self.graph.node_weight(*idx))
    }

    pub fn find_definition(&self, reference_id: &str) -> Option<&Symbol> {
        if let Some(&node_idx) = self.symbol_index.get(reference_id) {
            for edge in self
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
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
                && symbol.range.end.character >= position.character
            {
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
            for edge in self
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
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
            for edge in self
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
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

    /// Find references to a symbol (returns Symbol objects, not just references)
    pub fn find_references(&self, symbol_id: &str) -> anyhow::Result<Vec<Symbol>> {
        if let Some(&node_idx) = self.symbol_index.get(symbol_id) {
            let mut references = Vec::new();
            for edge in self
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
                if matches!(edge.weight(), EdgeKind::Reference) {
                    if let Some(symbol) = self.graph.node_weight(edge.source()) {
                        references.push(symbol.clone());
                    }
                }
            }
            Ok(references)
        } else {
            Ok(Vec::new())
        }
    }

    /// Find symbol at a specific position in a file
    pub fn find_symbol_at_position(
        &self,
        file_path: &str,
        position: Position,
    ) -> anyhow::Result<Option<Symbol>> {
        for symbol in self.get_all_symbols() {
            if symbol.file_path == file_path
                && position.line >= symbol.range.start.line
                && position.line <= symbol.range.end.line
            {
                if position.line == symbol.range.start.line
                    && position.character < symbol.range.start.character
                {
                    continue;
                }
                if position.line == symbol.range.end.line
                    && position.character > symbol.range.end.character
                {
                    continue;
                }
                return Ok(Some(symbol.clone()));
            }
        }
        Ok(None)
    }

    /// Get all symbols in a specific file
    pub fn get_symbols_in_file(&self, file_path: &str) -> anyhow::Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        for symbol in self.get_all_symbols() {
            if symbol.file_path == file_path {
                symbols.push(symbol.clone());
            }
        }
        Ok(symbols)
    }

    /// Get outgoing edges from a symbol
    pub fn get_outgoing_edges(
        &self,
        symbol_id: &str,
        edge_type: Option<EdgeKind>,
    ) -> anyhow::Result<Vec<Symbol>> {
        if let Some(&node_idx) = self.symbol_index.get(symbol_id) {
            let mut targets = Vec::new();
            for edge in self
                .graph
                .edges_directed(node_idx, petgraph::Direction::Outgoing)
            {
                if edge_type.is_none() || edge_type == Some(*edge.weight()) {
                    if let Some(symbol) = self.graph.node_weight(edge.target()) {
                        targets.push(symbol.clone());
                    }
                }
            }
            Ok(targets)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get incoming edges to a symbol
    pub fn get_incoming_edges(
        &self,
        symbol_id: &str,
        edge_type: Option<EdgeKind>,
    ) -> anyhow::Result<Vec<Symbol>> {
        if let Some(&node_idx) = self.symbol_index.get(symbol_id) {
            let mut sources = Vec::new();
            for edge in self
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
                if edge_type.is_none() || edge_type == Some(*edge.weight()) {
                    if let Some(symbol) = self.graph.node_weight(edge.source()) {
                        sources.push(symbol.clone());
                    }
                }
            }
            Ok(sources)
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_symbol(id: &str, name: &str, kind: SymbolKind) -> Symbol {
        Symbol {
            id: id.to_string(),
            kind,
            name: name.to_string(),
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
    fn test_new_graph() {
        let graph = CodeGraph::new();
        assert_eq!(graph.symbol_count(), 0);
    }

    #[test]
    fn test_add_symbol() {
        let mut graph = CodeGraph::new();
        let symbol = create_test_symbol("test1", "TestSymbol", SymbolKind::Function);
        let index = graph.add_symbol(symbol);

        assert_eq!(graph.symbol_count(), 1);
        assert!(graph.find_symbol("test1").is_some());
        assert!(graph.get_node_index("test1").is_some());
        assert_eq!(graph.get_node_index("test1"), Some(index));
    }

    #[test]
    fn test_remove_symbol() {
        let mut graph = CodeGraph::new();
        let symbol = create_test_symbol("test1", "TestSymbol", SymbolKind::Function);
        graph.add_symbol(symbol);

        assert!(graph.remove_symbol("test1"));
        assert_eq!(graph.symbol_count(), 0);
        assert!(graph.find_symbol("test1").is_none());

        // Removing non-existent symbol
        assert!(!graph.remove_symbol("nonexistent"));
    }

    #[test]
    fn test_add_edge() {
        let mut graph = CodeGraph::new();
        let symbol1 = create_test_symbol("func1", "Function1", SymbolKind::Function);
        let symbol2 = create_test_symbol("ref1", "Reference1", SymbolKind::Reference);

        let idx1 = graph.add_symbol(symbol1);
        let idx2 = graph.add_symbol(symbol2);

        graph.add_edge(idx2, idx1, EdgeKind::Reference);

        // The edge should exist in the graph
        let edges: Vec<_> = graph.graph.edges(idx2).collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(*edges[0].weight(), EdgeKind::Reference);
    }

    #[test]
    fn test_find_symbol() {
        let mut graph = CodeGraph::new();
        let symbol = create_test_symbol("test1", "TestSymbol", SymbolKind::Function);
        graph.add_symbol(symbol.clone());

        let found = graph.find_symbol("test1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "TestSymbol");

        assert!(graph.find_symbol("nonexistent").is_none());
    }

    #[test]
    fn test_find_references() {
        let mut graph = CodeGraph::new();
        let func = create_test_symbol("func1", "Function1", SymbolKind::Function);
        let ref1 = create_test_symbol("ref1", "Reference1", SymbolKind::Reference);
        let ref2 = create_test_symbol("ref2", "Reference2", SymbolKind::Reference);

        let func_idx = graph.add_symbol(func);
        let ref1_idx = graph.add_symbol(ref1);
        let ref2_idx = graph.add_symbol(ref2);

        // Add references pointing to the function
        graph.add_edge(ref1_idx, func_idx, EdgeKind::Reference);
        graph.add_edge(ref2_idx, func_idx, EdgeKind::Reference);

        let references = graph.find_references("func1").unwrap();
        assert_eq!(references.len(), 2);
    }

    #[test]
    fn test_find_definition() {
        let mut graph = CodeGraph::new();
        let func = create_test_symbol("func1", "Function1", SymbolKind::Function);
        let reference = create_test_symbol("ref1", "Reference1", SymbolKind::Reference);

        let func_idx = graph.add_symbol(func);
        let ref_idx = graph.add_symbol(reference);

        // Add definition edge from function to reference
        graph.add_edge(func_idx, ref_idx, EdgeKind::Definition);

        let definition = graph.find_definition("ref1");
        assert!(definition.is_some());
        assert_eq!(definition.unwrap().id, "func1");
    }

    #[test]
    fn test_find_definition_at() {
        let mut graph = CodeGraph::new();
        let mut symbol = create_test_symbol("func1", "Function1", SymbolKind::Function);
        symbol.file_path = "test.rs".to_string();
        symbol.range = Range {
            start: Position {
                line: 5,
                character: 10,
            },
            end: Position {
                line: 5,
                character: 20,
            },
        };

        graph.add_symbol(symbol);

        // Test finding at exact position
        let found = graph.find_definition_at(
            "test.rs",
            Position {
                line: 5,
                character: 15,
            },
        );
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "func1");

        // Test outside range
        let not_found = graph.find_definition_at(
            "test.rs",
            Position {
                line: 6,
                character: 15,
            },
        );
        assert!(not_found.is_none());

        // Test wrong file
        let wrong_file = graph.find_definition_at(
            "other.rs",
            Position {
                line: 5,
                character: 15,
            },
        );
        assert!(wrong_file.is_none());
    }

    #[test]
    fn test_find_implementations() {
        let mut graph = CodeGraph::new();
        let interface = create_test_symbol("interface1", "Interface1", SymbolKind::Interface);
        let impl1 = create_test_symbol("impl1", "Implementation1", SymbolKind::Class);
        let impl2 = create_test_symbol("impl2", "Implementation2", SymbolKind::Class);

        let interface_idx = graph.add_symbol(interface);
        let impl1_idx = graph.add_symbol(impl1);
        let impl2_idx = graph.add_symbol(impl2);

        // Add implementation edges
        graph.add_edge(impl1_idx, interface_idx, EdgeKind::Implementation);
        graph.add_edge(impl2_idx, interface_idx, EdgeKind::Implementation);

        let implementations = graph.find_implementations("interface1");
        assert_eq!(implementations.len(), 2);
    }

    #[test]
    fn test_find_overrides() {
        let mut graph = CodeGraph::new();
        let base_method = create_test_symbol("base_method", "BaseMethod", SymbolKind::Method);
        let override1 = create_test_symbol("override1", "Override1", SymbolKind::Method);
        let override2 = create_test_symbol("override2", "Override2", SymbolKind::Method);

        let base_idx = graph.add_symbol(base_method);
        let override1_idx = graph.add_symbol(override1);
        let override2_idx = graph.add_symbol(override2);

        // Add override edges
        graph.add_edge(override1_idx, base_idx, EdgeKind::Override);
        graph.add_edge(override2_idx, base_idx, EdgeKind::Override);

        let overrides = graph.find_overrides("base_method");
        assert_eq!(overrides.len(), 2);
    }

    #[test]
    fn test_get_all_symbols() {
        let mut graph = CodeGraph::new();
        let symbol1 = create_test_symbol("sym1", "Symbol1", SymbolKind::Function);
        let symbol2 = create_test_symbol("sym2", "Symbol2", SymbolKind::Variable);
        let symbol3 = create_test_symbol("sym3", "Symbol3", SymbolKind::Class);

        graph.add_symbol(symbol1);
        graph.add_symbol(symbol2);
        graph.add_symbol(symbol3);

        let all_symbols: Vec<_> = graph.get_all_symbols().collect();
        assert_eq!(all_symbols.len(), 3);
    }

    #[test]
    fn test_symbol_count() {
        let mut graph = CodeGraph::new();
        assert_eq!(graph.symbol_count(), 0);

        graph.add_symbol(create_test_symbol("sym1", "Symbol1", SymbolKind::Function));
        assert_eq!(graph.symbol_count(), 1);

        graph.add_symbol(create_test_symbol("sym2", "Symbol2", SymbolKind::Variable));
        assert_eq!(graph.symbol_count(), 2);

        graph.remove_symbol("sym1");
        assert_eq!(graph.symbol_count(), 1);
    }

    #[test]
    fn test_edge_kinds() {
        // Test that all edge kinds are distinct
        assert_ne!(EdgeKind::Definition, EdgeKind::Reference);
        assert_ne!(EdgeKind::TypeDefinition, EdgeKind::Implementation);
        assert_ne!(EdgeKind::Override, EdgeKind::Import);
        assert_ne!(EdgeKind::Export, EdgeKind::Contains);
    }

    #[test]
    fn test_symbol_kinds() {
        // Test that symbol kinds are properly defined
        assert_ne!(SymbolKind::Function, SymbolKind::Method);
        assert_ne!(SymbolKind::Class, SymbolKind::Interface);
        assert_ne!(SymbolKind::Variable, SymbolKind::Constant);
    }

    #[test]
    fn test_position_and_range() {
        let pos1 = Position {
            line: 10,
            character: 5,
        };
        let pos2 = Position {
            line: 10,
            character: 15,
        };
        let range = Range {
            start: pos1,
            end: pos2,
        };

        assert_eq!(range.start.line, 10);
        assert_eq!(range.start.character, 5);
        assert_eq!(range.end.line, 10);
        assert_eq!(range.end.character, 15);
    }

    #[test]
    fn test_find_definition_with_reference_edge() {
        let mut graph = CodeGraph::new();
        let func = create_test_symbol("func1", "Function1", SymbolKind::Function);
        let mut reference = create_test_symbol("ref1", "Reference1", SymbolKind::Reference);
        reference.range = Range {
            start: Position {
                line: 10,
                character: 5,
            },
            end: Position {
                line: 10,
                character: 15,
            },
        };

        let func_idx = graph.add_symbol(func);
        let ref_idx = graph.add_symbol(reference);

        // Add reference edge from reference to function
        graph.add_edge(ref_idx, func_idx, EdgeKind::Reference);

        // Find definition at reference position
        let definition = graph.find_definition_at(
            "test.rs",
            Position {
                line: 10,
                character: 10,
            },
        );
        assert!(definition.is_some());
        assert_eq!(definition.unwrap().id, "func1");
    }

    #[test]
    fn test_empty_graph_operations() {
        let graph = CodeGraph::new();

        assert!(graph.find_symbol("nonexistent").is_none());
        assert_eq!(graph.find_references("nonexistent").unwrap().len(), 0);
        assert!(graph.find_definition("nonexistent").is_none());
        assert_eq!(graph.find_implementations("nonexistent").len(), 0);
        assert_eq!(graph.find_overrides("nonexistent").len(), 0);
        assert!(graph.get_node_index("nonexistent").is_none());
        assert_eq!(graph.get_all_symbols().count(), 0);
    }
}
// Test comment

// Test comment for differential index
// Test change for differential indexing
// Test comment for differential
