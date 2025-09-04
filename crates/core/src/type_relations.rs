use super::graph::{CodeGraph, EdgeKind, Symbol, SymbolKind};
use petgraph::visit::EdgeRef;
use std::collections::{HashSet, VecDeque};

/// Result of collecting type-related symbols
#[derive(Debug, Clone)]
pub struct TypeRelations {
    /// The root type symbol
    pub root_type: Symbol,
    /// All symbols that use this type
    pub users: Vec<Symbol>,
    /// All symbols that implement this type (for interfaces/traits)
    pub implementations: Vec<Symbol>,
    /// All symbols that extend/inherit from this type
    pub extensions: Vec<Symbol>,
    /// All fields/properties of this type
    pub members: Vec<Symbol>,
    /// All methods of this type
    pub methods: Vec<Symbol>,
    /// All type parameters/generics used by this type
    pub type_parameters: Vec<Symbol>,
    /// Total count of related symbols
    pub total_relations: usize,
}

/// Analyzer for collecting type-related symbols recursively
pub struct TypeRelationsAnalyzer<'a> {
    graph: &'a CodeGraph,
}

impl<'a> TypeRelationsAnalyzer<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// Collect all symbols related to a type recursively
    pub fn collect_type_relations(
        &self,
        type_symbol_id: &str,
        max_depth: usize,
    ) -> Option<TypeRelations> {
        let root_type = self.graph.find_symbol(type_symbol_id)?.clone();

        // Check if it's actually a type-like symbol
        if !self.is_type_symbol(&root_type) {
            return None;
        }

        let mut users = Vec::new();
        let mut implementations = Vec::new();
        let mut extensions = Vec::new();
        let mut members = Vec::new();
        let mut methods = Vec::new();
        let mut type_parameters = Vec::new();
        let mut visited = HashSet::new();

        // Recursively collect all related symbols
        self.collect_recursive(
            type_symbol_id,
            &mut users,
            &mut implementations,
            &mut extensions,
            &mut members,
            &mut methods,
            &mut type_parameters,
            &mut visited,
            0,
            max_depth,
        );

        let total_relations = users.len()
            + implementations.len()
            + extensions.len()
            + members.len()
            + methods.len()
            + type_parameters.len();

        Some(TypeRelations {
            root_type,
            users,
            implementations,
            extensions,
            members,
            methods,
            type_parameters,
            total_relations,
        })
    }

    /// Find all symbols that reference a type (recursively)
    pub fn find_all_type_references(&self, type_symbol_id: &str, max_depth: usize) -> Vec<Symbol> {
        let mut all_references = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((type_symbol_id.to_string(), 0));
        visited.insert(type_symbol_id.to_string());

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth > max_depth {
                continue;
            }

            // Get direct references
            let refs = self.graph.find_references(&current_id).unwrap_or_default();

            for reference in refs {
                if visited.insert(reference.id.clone()) {
                    all_references.push(reference);

                    // Add to queue for recursive search
                    if depth < max_depth {
                        queue.push_back((all_references.last().unwrap().id.clone(), depth + 1));
                    }
                }
            }

            // Also check for symbols that have this type
            if let Some(node_idx) = self.graph.get_node_index(&current_id) {
                // Look for incoming edges
                for edge in self
                    .graph
                    .graph
                    .edges_directed(node_idx, petgraph::Direction::Incoming)
                {
                    if matches!(edge.weight(), EdgeKind::Reference | EdgeKind::Definition) {
                        if let Some(source) = self.graph.graph.node_weight(edge.source()) {
                            if visited.insert(source.id.clone()) {
                                all_references.push(source.clone());

                                if depth < max_depth {
                                    queue.push_back((source.id.clone(), depth + 1));
                                }
                            }
                        }
                    }
                }
            }
        }

        all_references
    }

    /// Find all types that are related through inheritance/implementation
    pub fn find_type_hierarchy(&self, type_symbol_id: &str) -> TypeHierarchy {
        let mut hierarchy = TypeHierarchy::new();

        if let Some(root) = self.graph.find_symbol(type_symbol_id) {
            hierarchy.root = Some(root.clone());

            // Find parents (what this type extends/implements)
            self.find_parent_types(type_symbol_id, &mut hierarchy.parents, &mut HashSet::new());

            // Find children (what extends/implements this type)
            self.find_child_types(type_symbol_id, &mut hierarchy.children, &mut HashSet::new());

            // Find siblings (types that share a parent with this type)
            for parent in &hierarchy.parents {
                self.find_child_types(&parent.id, &mut hierarchy.siblings, &mut HashSet::new());
            }

            // Remove self from siblings
            hierarchy.siblings.retain(|s| s.id != type_symbol_id);
        }

        hierarchy
    }

    /// Group related symbols by their relationship type
    pub fn group_relations_by_type(&self, type_symbol_id: &str) -> RelationGroups {
        let mut groups = RelationGroups::default();

        if let Some(node_idx) = self.graph.get_node_index(type_symbol_id) {
            // Analyze outgoing edges
            for edge in self.graph.graph.edges(node_idx) {
                if let Some(target) = self.graph.graph.node_weight(edge.target()) {
                    match edge.weight() {
                        EdgeKind::Definition => {
                            groups.definitions.push(target.clone());
                        }
                        EdgeKind::Reference => {
                            groups.references.push(target.clone());
                        }
                        _ => {}
                    }
                }
            }

            // Analyze incoming edges
            for edge in self
                .graph
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
                if let Some(source) = self.graph.graph.node_weight(edge.source()) {
                    match edge.weight() {
                        EdgeKind::Definition => {
                            groups.defined_by.push(source.clone());
                        }
                        EdgeKind::Reference => {
                            groups.referenced_by.push(source.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        // Categorize by symbol kind
        for symbol in &groups.referenced_by {
            match symbol.kind {
                SymbolKind::Variable | SymbolKind::Parameter => {
                    groups.variables_of_type.push(symbol.clone());
                }
                SymbolKind::Function | SymbolKind::Method => {
                    groups.functions_returning_type.push(symbol.clone());
                }
                SymbolKind::Property | SymbolKind::Field => {
                    groups.fields_of_type.push(symbol.clone());
                }
                _ => {}
            }
        }

        groups
    }

    // Helper methods

    fn is_type_symbol(&self, symbol: &Symbol) -> bool {
        matches!(
            symbol.kind,
            SymbolKind::Class
                | SymbolKind::Interface
                | SymbolKind::Enum
                | SymbolKind::Module
                | SymbolKind::Namespace
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_recursive(
        &self,
        symbol_id: &str,
        users: &mut Vec<Symbol>,
        implementations: &mut Vec<Symbol>,
        extensions: &mut Vec<Symbol>,
        members: &mut Vec<Symbol>,
        methods: &mut Vec<Symbol>,
        type_parameters: &mut Vec<Symbol>,
        visited: &mut HashSet<String>,
        current_depth: usize,
        max_depth: usize,
    ) {
        if current_depth > max_depth || !visited.insert(symbol_id.to_string()) {
            return;
        }

        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            // Collect incoming references (who uses this type)
            for edge in self
                .graph
                .graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
            {
                if let Some(source) = self.graph.graph.node_weight(edge.source()) {
                    match source.kind {
                        SymbolKind::Variable | SymbolKind::Parameter => {
                            users.push(source.clone());
                        }
                        SymbolKind::Class | SymbolKind::Interface => {
                            // Check if it's extending or implementing
                            if matches!(edge.weight(), EdgeKind::Definition) {
                                extensions.push(source.clone());
                            } else {
                                implementations.push(source.clone());
                            }
                        }
                        SymbolKind::Method => {
                            methods.push(source.clone());
                        }
                        SymbolKind::Property | SymbolKind::Field => {
                            members.push(source.clone());
                        }
                        _ => {}
                    }

                    // Recurse
                    if current_depth < max_depth {
                        self.collect_recursive(
                            &source.id,
                            users,
                            implementations,
                            extensions,
                            members,
                            methods,
                            type_parameters,
                            visited,
                            current_depth + 1,
                            max_depth,
                        );
                    }
                }
            }

            // Collect outgoing references (what this type depends on)
            for edge in self.graph.graph.edges(node_idx) {
                if let Some(target) = self.graph.graph.node_weight(edge.target()) {
                    if self.is_type_symbol(target) {
                        type_parameters.push(target.clone());
                    }
                }
            }
        }
    }

    /// 汎用的な型関係探索メソッド
    fn find_related_types(
        &self,
        symbol_id: &str,
        direction: petgraph::Direction,
        results: &mut Vec<Symbol>,
        visited: &mut HashSet<String>,
    ) {
        if !visited.insert(symbol_id.to_string()) {
            return;
        }

        if let Some(node_idx) = self.graph.get_node_index(symbol_id) {
            for edge in self.graph.graph.edges_directed(node_idx, direction) {
                if matches!(edge.weight(), EdgeKind::Definition) {
                    let related_node = match direction {
                        petgraph::Direction::Outgoing => edge.target(),
                        petgraph::Direction::Incoming => edge.source(),
                    };

                    if let Some(related) = self.graph.graph.node_weight(related_node) {
                        if self.is_type_symbol(related) {
                            results.push(related.clone());
                            self.find_related_types(&related.id, direction, results, visited);
                        }
                    }
                }
            }
        }
    }

    fn find_parent_types(
        &self,
        symbol_id: &str,
        parents: &mut Vec<Symbol>,
        visited: &mut HashSet<String>,
    ) {
        self.find_related_types(symbol_id, petgraph::Direction::Outgoing, parents, visited);
    }

    fn find_child_types(
        &self,
        symbol_id: &str,
        children: &mut Vec<Symbol>,
        visited: &mut HashSet<String>,
    ) {
        self.find_related_types(symbol_id, petgraph::Direction::Incoming, children, visited);
    }
}

/// Type hierarchy information
#[derive(Debug, Clone)]
pub struct TypeHierarchy {
    pub root: Option<Symbol>,
    pub parents: Vec<Symbol>,
    pub children: Vec<Symbol>,
    pub siblings: Vec<Symbol>,
}

impl TypeHierarchy {
    fn new() -> Self {
        Self {
            root: None,
            parents: Vec::new(),
            children: Vec::new(),
            siblings: Vec::new(),
        }
    }
}

/// Relations grouped by type
#[derive(Debug, Clone, Default)]
pub struct RelationGroups {
    pub definitions: Vec<Symbol>,
    pub references: Vec<Symbol>,
    pub defined_by: Vec<Symbol>,
    pub referenced_by: Vec<Symbol>,
    pub variables_of_type: Vec<Symbol>,
    pub functions_returning_type: Vec<Symbol>,
    pub fields_of_type: Vec<Symbol>,
}

/// Format type relations as a report
pub fn format_type_relations(relations: &TypeRelations) -> String {
    let mut report = String::new();

    report.push_str(&format!(
        "Type: {} ({})\n",
        relations.root_type.name, relations.root_type.file_path
    ));
    report.push_str(&format!(
        "Total related symbols: {}\n\n",
        relations.total_relations
    ));

    if !relations.users.is_empty() {
        report.push_str(&format!("Used by {} symbols:\n", relations.users.len()));
        for (i, user) in relations.users.iter().take(5).enumerate() {
            report.push_str(&format!(
                "  {}. {} ({})\n",
                i + 1,
                user.name,
                user.file_path
            ));
        }
        if relations.users.len() > 5 {
            report.push_str(&format!("  ... and {} more\n", relations.users.len() - 5));
        }
        report.push('\n');
    }

    if !relations.implementations.is_empty() {
        report.push_str(&format!(
            "Implemented by {} types:\n",
            relations.implementations.len()
        ));
        for impl_type in relations.implementations.iter().take(5) {
            report.push_str(&format!(
                "  - {} ({})\n",
                impl_type.name, impl_type.file_path
            ));
        }
        report.push('\n');
    }

    if !relations.extensions.is_empty() {
        report.push_str(&format!(
            "Extended by {} types:\n",
            relations.extensions.len()
        ));
        for ext_type in relations.extensions.iter().take(5) {
            report.push_str(&format!("  - {} ({})\n", ext_type.name, ext_type.file_path));
        }
        report.push('\n');
    }

    if !relations.members.is_empty() {
        report.push_str(&format!("Has {} members:\n", relations.members.len()));
        for member in relations.members.iter().take(5) {
            report.push_str(&format!("  - {} ({:?})\n", member.name, member.kind));
        }
        report.push('\n');
    }

    if !relations.methods.is_empty() {
        report.push_str(&format!("Has {} methods:\n", relations.methods.len()));
        for method in relations.methods.iter().take(5) {
            report.push_str(&format!("  - {}\n", method.name));
        }
        report.push('\n');
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Position, Range};

    fn create_type_symbol(id: &str, name: &str, kind: SymbolKind) -> Symbol {
        Symbol {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            file_path: "test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 10,
                    character: 0,
                },
            },
            documentation: None,
            detail: None,
        }
    }

    #[test]
    fn test_collect_type_relations() {
        let mut graph = CodeGraph::new();

        // Create a type hierarchy
        let base_type = create_type_symbol("type:Base", "BaseClass", SymbolKind::Class);
        let derived_type = create_type_symbol("type:Derived", "DerivedClass", SymbolKind::Class);
        let var1 = create_type_symbol("var:x", "x", SymbolKind::Variable);
        let var2 = create_type_symbol("var:y", "y", SymbolKind::Variable);
        let method1 = create_type_symbol("method:foo", "foo", SymbolKind::Method);

        let base_idx = graph.add_symbol(base_type.clone());
        let derived_idx = graph.add_symbol(derived_type);
        let var1_idx = graph.add_symbol(var1.clone());
        let var2_idx = graph.add_symbol(var2.clone());
        let method_idx = graph.add_symbol(method1.clone());

        // Derived extends Base
        graph.add_edge(derived_idx, base_idx, EdgeKind::Definition);

        // Variables reference Base type
        graph.add_edge(var1_idx, base_idx, EdgeKind::Reference);
        graph.add_edge(var2_idx, base_idx, EdgeKind::Reference);

        // Method belongs to Base
        graph.add_edge(method_idx, base_idx, EdgeKind::Reference);

        let analyzer = TypeRelationsAnalyzer::new(&graph);
        let relations = analyzer.collect_type_relations("type:Base", 2).unwrap();

        assert_eq!(relations.root_type.id, "type:Base");
        assert_eq!(relations.users.len(), 2); // var1 and var2
        assert_eq!(relations.extensions.len(), 1); // Derived
        assert_eq!(relations.methods.len(), 1); // foo
    }

    #[test]
    fn test_find_all_type_references() {
        let mut graph = CodeGraph::new();

        // Create a chain of references
        let type_a = create_type_symbol("type:A", "TypeA", SymbolKind::Class);
        let type_b = create_type_symbol("type:B", "TypeB", SymbolKind::Class);
        let var_x = create_type_symbol("var:x", "x", SymbolKind::Variable);
        let var_y = create_type_symbol("var:y", "y", SymbolKind::Variable);

        let a_idx = graph.add_symbol(type_a);
        let b_idx = graph.add_symbol(type_b);
        let x_idx = graph.add_symbol(var_x);
        let y_idx = graph.add_symbol(var_y);

        // x -> A, y -> B, B -> A
        graph.add_edge(x_idx, a_idx, EdgeKind::Reference);
        graph.add_edge(y_idx, b_idx, EdgeKind::Reference);
        graph.add_edge(b_idx, a_idx, EdgeKind::Reference);

        let analyzer = TypeRelationsAnalyzer::new(&graph);
        let refs = analyzer.find_all_type_references("type:A", 2);

        // Should find x directly and y through B
        assert!(refs.len() >= 2);
        assert!(refs.iter().any(|r| r.id == "var:x"));
        assert!(refs.iter().any(|r| r.id == "type:B"));
    }

    #[test]
    fn test_type_hierarchy() {
        let mut graph = CodeGraph::new();

        // Create hierarchy: Interface <- Base <- Derived1, Derived2
        let interface = create_type_symbol("type:Interface", "IBase", SymbolKind::Interface);
        let base = create_type_symbol("type:Base", "BaseClass", SymbolKind::Class);
        let derived1 = create_type_symbol("type:Derived1", "Derived1", SymbolKind::Class);
        let derived2 = create_type_symbol("type:Derived2", "Derived2", SymbolKind::Class);

        let interface_idx = graph.add_symbol(interface);
        let base_idx = graph.add_symbol(base);
        let derived1_idx = graph.add_symbol(derived1);
        let derived2_idx = graph.add_symbol(derived2);

        // Base implements Interface
        graph.add_edge(base_idx, interface_idx, EdgeKind::Definition);

        // Derived1 and Derived2 extend Base
        graph.add_edge(derived1_idx, base_idx, EdgeKind::Definition);
        graph.add_edge(derived2_idx, base_idx, EdgeKind::Definition);

        let analyzer = TypeRelationsAnalyzer::new(&graph);
        let hierarchy = analyzer.find_type_hierarchy("type:Base");

        assert!(hierarchy.root.is_some());
        assert_eq!(hierarchy.parents.len(), 1); // Interface
        assert_eq!(hierarchy.children.len(), 2); // Derived1, Derived2
    }

    #[test]
    fn test_relation_groups() {
        let mut graph = CodeGraph::new();

        // Create various relations
        let my_type = create_type_symbol("type:MyType", "MyType", SymbolKind::Class);
        let var = create_type_symbol("var:x", "x", SymbolKind::Variable);
        let func = create_type_symbol("fn:getMyType", "getMyType", SymbolKind::Function);
        let field = create_type_symbol("field:data", "data", SymbolKind::Field);

        let type_idx = graph.add_symbol(my_type);
        let var_idx = graph.add_symbol(var);
        let func_idx = graph.add_symbol(func);
        let field_idx = graph.add_symbol(field);

        // Variable of MyType
        graph.add_edge(var_idx, type_idx, EdgeKind::Reference);

        // Function returns MyType
        graph.add_edge(func_idx, type_idx, EdgeKind::Reference);

        // Field of MyType
        graph.add_edge(field_idx, type_idx, EdgeKind::Reference);

        let analyzer = TypeRelationsAnalyzer::new(&graph);
        let groups = analyzer.group_relations_by_type("type:MyType");

        assert!(!groups.referenced_by.is_empty());
        assert_eq!(groups.variables_of_type.len(), 1);
        assert_eq!(groups.functions_returning_type.len(), 1);
        assert_eq!(groups.fields_of_type.len(), 1);
    }
}
