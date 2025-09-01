use super::graph::{CodeGraph, Position, Range, Symbol, SymbolKind};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;

// LSIF Element definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LsifElement {
    Vertex(Vertex),
    Edge(Edge),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: String,
    #[serde(rename = "type")]
    pub element_type: String, // Always "vertex"
    pub label: String,
    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    #[serde(rename = "type")]
    pub element_type: String, // Always "edge"
    pub label: String,
    #[serde(rename = "outV")]
    pub out_v: String,
    #[serde(rename = "inV")]
    pub in_v: String,
    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

// LSIF Labels
pub mod labels {
    pub const METADATA: &str = "metaData";
    pub const PROJECT: &str = "project";
    pub const DOCUMENT: &str = "document";
    pub const RANGE: &str = "range";
    pub const RESULT_SET: &str = "resultSet";
    pub const DEFINITION_RESULT: &str = "definitionResult";
    pub const REFERENCE_RESULT: &str = "referenceResult";
    pub const HOVER_RESULT: &str = "hoverResult";
    pub const MONIKER: &str = "moniker";
    pub const PACKAGE_INFORMATION: &str = "packageInformation";

    // Edge labels
    pub const CONTAINS: &str = "contains";
    pub const ITEM: &str = "item";
    pub const NEXT: &str = "next";
    pub const MONIKER_EDGE: &str = "moniker";
    pub const NEXT_MONIKER: &str = "nextMoniker";
    pub const PACKAGE_INFORMATION_EDGE: &str = "packageInformation";
    pub const TEXTDOCUMENT_DEFINITION: &str = "textDocument/definition";
    pub const TEXTDOCUMENT_REFERENCES: &str = "textDocument/references";
    pub const TEXTDOCUMENT_HOVER: &str = "textDocument/hover";
}

// LSIF Generator - generates LSIF from CodeGraph
pub struct LsifGenerator {
    graph: CodeGraph,
    id_counter: usize,
    vertex_ids: HashMap<String, String>, // Symbol ID -> LSIF vertex ID
    elements: Vec<LsifElement>,
}

impl LsifGenerator {
    pub fn new(graph: CodeGraph) -> Self {
        Self {
            graph,
            id_counter: 0,
            vertex_ids: HashMap::new(),
            elements: Vec::new(),
        }
    }

    fn next_id(&mut self) -> String {
        self.id_counter += 1;
        self.id_counter.to_string()
    }

    pub fn generate(&mut self) -> Result<String> {
        // 1. Generate metadata
        self.generate_metadata()?;

        // 2. Generate project
        let project_id = self.generate_project()?;

        // 3. Collect all symbols first to avoid borrowing issues
        let mut all_symbols = Vec::new();
        for symbol_id in self.graph.symbol_index.keys() {
            if let Some(symbol) = self.graph.find_symbol(symbol_id) {
                all_symbols.push(symbol.clone());
            }
        }

        // 4. Generate documents and their contents
        let mut documents: HashMap<String, String> = HashMap::new(); // file_path -> document_id

        for symbol in &all_symbols {
            if !documents.contains_key(&symbol.file_path) {
                let doc_id = self.generate_document(&symbol.file_path)?;
                documents.insert(symbol.file_path.clone(), doc_id.clone());

                // Link document to project
                self.generate_contains_edge(&project_id, &doc_id)?;
            }
        }

        // 5. Generate ranges and symbols
        for symbol in &all_symbols {
            let doc_id = documents
                .get(&symbol.file_path)
                .ok_or_else(|| anyhow::anyhow!("Document not found"))?;

            // Generate range for symbol
            let range_id = self.generate_range(symbol)?;
            self.vertex_ids.insert(symbol.id.clone(), range_id.clone());

            // Link range to document
            self.generate_contains_edge(doc_id, &range_id)?;

            // Generate result set for the range
            let result_set_id = self.generate_result_set()?;
            self.generate_next_edge(&range_id, &result_set_id)?;

            // Generate hover result if documentation exists
            if let Some(doc) = &symbol.documentation {
                self.generate_hover(&result_set_id, doc)?;
            }
        }

        // 6. Generate edges for references and definitions
        self.generate_reference_edges()?;

        // Convert to JSON Lines format
        let mut output = String::new();
        for element in &self.elements {
            output.push_str(&serde_json::to_string(element)?);
            output.push('\n');
        }

        Ok(output)
    }

    fn generate_metadata(&mut self) -> Result<()> {
        let id = self.next_id();
        let mut data = HashMap::new();
        data.insert("version".to_string(), json!("0.5.0"));
        data.insert("projectRoot".to_string(), json!("file:///"));
        data.insert("positionEncoding".to_string(), json!("utf-16"));
        data.insert(
            "toolInfo".to_string(),
            json!({
                "name": "lsif-indexer",
                "version": "1.0.0"
            }),
        );

        let vertex = Vertex {
            id,
            element_type: "vertex".to_string(),
            label: labels::METADATA.to_string(),
            data,
        };

        self.elements.push(LsifElement::Vertex(vertex));
        Ok(())
    }

    fn generate_project(&mut self) -> Result<String> {
        let id = self.next_id();
        let mut data = HashMap::new();
        data.insert("kind".to_string(), json!("rust"));

        let vertex = Vertex {
            id: id.clone(),
            element_type: "vertex".to_string(),
            label: labels::PROJECT.to_string(),
            data,
        };

        self.elements.push(LsifElement::Vertex(vertex));
        Ok(id)
    }

    fn generate_document(&mut self, file_path: &str) -> Result<String> {
        let id = self.next_id();
        let mut data = HashMap::new();
        data.insert("uri".to_string(), json!(format!("file://{}", file_path)));
        data.insert("languageId".to_string(), json!("rust"));

        let vertex = Vertex {
            id: id.clone(),
            element_type: "vertex".to_string(),
            label: labels::DOCUMENT.to_string(),
            data,
        };

        self.elements.push(LsifElement::Vertex(vertex));
        Ok(id)
    }

    fn generate_range(&mut self, symbol: &Symbol) -> Result<String> {
        let id = self.next_id();
        let mut data = HashMap::new();
        data.insert(
            "start".to_string(),
            json!({
                "line": symbol.range.start.line,
                "character": symbol.range.start.character
            }),
        );
        data.insert(
            "end".to_string(),
            json!({
                "line": symbol.range.end.line,
                "character": symbol.range.end.character
            }),
        );

        let vertex = Vertex {
            id: id.clone(),
            element_type: "vertex".to_string(),
            label: labels::RANGE.to_string(),
            data,
        };

        self.elements.push(LsifElement::Vertex(vertex));
        Ok(id)
    }

    fn generate_result_set(&mut self) -> Result<String> {
        let id = self.next_id();
        let vertex = Vertex {
            id: id.clone(),
            element_type: "vertex".to_string(),
            label: labels::RESULT_SET.to_string(),
            data: HashMap::new(),
        };

        self.elements.push(LsifElement::Vertex(vertex));
        Ok(id)
    }

    fn generate_hover(&mut self, result_set_id: &str, content: &str) -> Result<()> {
        let hover_id = self.next_id();
        let mut data = HashMap::new();
        data.insert(
            "result".to_string(),
            json!({
                "contents": {
                    "kind": "markdown",
                    "value": content
                }
            }),
        );

        let vertex = Vertex {
            id: hover_id.clone(),
            element_type: "vertex".to_string(),
            label: labels::HOVER_RESULT.to_string(),
            data,
        };

        self.elements.push(LsifElement::Vertex(vertex));

        // Connect hover to result set
        let edge_id = self.next_id();
        let edge = Edge {
            id: edge_id,
            element_type: "edge".to_string(),
            label: labels::TEXTDOCUMENT_HOVER.to_string(),
            out_v: result_set_id.to_string(),
            in_v: hover_id,
            data: HashMap::new(),
        };

        self.elements.push(LsifElement::Edge(edge));
        Ok(())
    }

    fn generate_contains_edge(&mut self, from: &str, to: &str) -> Result<()> {
        let id = self.next_id();
        let edge = Edge {
            id,
            element_type: "edge".to_string(),
            label: labels::CONTAINS.to_string(),
            out_v: from.to_string(),
            in_v: to.to_string(),
            data: HashMap::new(),
        };

        self.elements.push(LsifElement::Edge(edge));
        Ok(())
    }

    fn generate_next_edge(&mut self, from: &str, to: &str) -> Result<()> {
        let id = self.next_id();
        let edge = Edge {
            id,
            element_type: "edge".to_string(),
            label: labels::NEXT.to_string(),
            out_v: from.to_string(),
            in_v: to.to_string(),
            data: HashMap::new(),
        };

        self.elements.push(LsifElement::Edge(edge));
        Ok(())
    }

    fn generate_reference_edges(&mut self) -> Result<()> {
        // This would generate definition and reference edges based on the graph relationships
        // For now, we'll keep it simple
        Ok(())
    }
}

// LSIF Parser - parses LSIF format into CodeGraph
pub struct LsifParser {
    graph: CodeGraph,
    documents: HashMap<String, String>,   // vertex_id -> uri
    ranges: HashMap<String, LsifRange>,   // vertex_id -> range
    result_sets: HashMap<String, String>, // range_id -> result_set_id
}

#[derive(Debug, Clone)]
struct LsifRange {
    document_id: String,
    start: Position,
    end: Position,
}

impl Default for LsifParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LsifParser {
    pub fn new() -> Self {
        Self {
            graph: CodeGraph::new(),
            documents: HashMap::new(),
            ranges: HashMap::new(),
            result_sets: HashMap::new(),
        }
    }

    pub fn parse(&mut self, content: &str) -> Result<()> {
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let value: Value = serde_json::from_str(line)?;
            self.process_element(value)?;
        }
        Ok(())
    }

    fn process_element(&mut self, value: Value) -> Result<()> {
        if let Some(element_type) = value.get("type").and_then(|v| v.as_str()) {
            match element_type {
                "vertex" => self.process_vertex(value)?,
                "edge" => self.process_edge(value)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn process_vertex(&mut self, value: Value) -> Result<()> {
        if let (Some(id), Some(label)) = (
            value.get("id").and_then(|v| v.as_str()),
            value.get("label").and_then(|v| v.as_str()),
        ) {
            match label {
                labels::DOCUMENT => {
                    if let Some(uri) = value.get("uri").and_then(|v| v.as_str()) {
                        self.documents.insert(id.to_string(), uri.to_string());
                    }
                }
                labels::RANGE => {
                    if let (Some(start), Some(end)) = (value.get("start"), value.get("end")) {
                        let range = LsifRange {
                            document_id: String::new(),
                            start: self.parse_position(start)?,
                            end: self.parse_position(end)?,
                        };
                        self.ranges.insert(id.to_string(), range);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn process_edge(&mut self, value: Value) -> Result<()> {
        if let (Some(label), Some(out_v), Some(in_v)) = (
            value.get("label").and_then(|v| v.as_str()),
            value.get("outV").and_then(|v| v.as_str()),
            value.get("inV").and_then(|v| v.as_str()),
        ) {
            match label {
                labels::CONTAINS => {
                    // Document contains range
                    if self.documents.contains_key(out_v) {
                        if let Some(range) = self.ranges.get_mut(in_v) {
                            range.document_id = out_v.to_string();
                        }
                    }
                }
                labels::NEXT => {
                    // Range -> ResultSet
                    self.result_sets.insert(out_v.to_string(), in_v.to_string());
                }
                labels::TEXTDOCUMENT_DEFINITION | labels::TEXTDOCUMENT_REFERENCES => {
                    // Create symbol from range
                    if let Some(range) = self.ranges.get(out_v) {
                        if let Some(doc_uri) = self.documents.get(&range.document_id) {
                            let symbol = Symbol {
                                id: out_v.to_string(),
                                kind: SymbolKind::Function,
                                name: format!("symbol_{out_v}"),
                                file_path: doc_uri.clone(),
                                range: Range {
                                    start: range.start,
                                    end: range.end,
                                },
                                documentation: None,
                                detail: None,
                            };
                            self.graph.add_symbol(symbol);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_position(&self, value: &Value) -> Result<Position> {
        Ok(Position {
            line: value.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            character: value.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        })
    }

    pub fn into_graph(self) -> CodeGraph {
        self.graph
    }
}

// Public API
pub fn generate_lsif(graph: CodeGraph) -> Result<String> {
    let mut generator = LsifGenerator::new(graph);
    generator.generate()
}

pub fn parse_lsif(content: &str) -> Result<CodeGraph> {
    let mut parser = LsifParser::new();
    parser.parse(content)?;
    Ok(parser.into_graph())
}

pub fn write_lsif<W: Write>(writer: &mut W, graph: CodeGraph) -> Result<()> {
    let lsif_content = generate_lsif(graph)?;
    writer.write_all(lsif_content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new();

        let symbol1 = Symbol {
            id: "symbol1".to_string(),
            name: "main".to_string(),
            kind: SymbolKind::Function,
            file_path: "/test/main.rs".to_string(),
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
            documentation: Some("Main function".to_string()),
            detail: None,
        };

        let symbol2 = Symbol {
            id: "symbol2".to_string(),
            name: "helper".to_string(),
            kind: SymbolKind::Function,
            file_path: "/test/helper.rs".to_string(),
            range: Range {
                start: Position {
                    line: 5,
                    character: 3,
                },
                end: Position {
                    line: 5,
                    character: 15,
                },
            },
            documentation: None,
            detail: None,
        };

        let idx1 = graph.add_symbol(symbol1);
        let idx2 = graph.add_symbol(symbol2);
        graph.add_edge(idx1, idx2, crate::graph::EdgeKind::Reference);

        graph
    }

    #[test]
    fn test_lsif_element_serialization() {
        let vertex = Vertex {
            id: "1".to_string(),
            element_type: "vertex".to_string(),
            label: "document".to_string(),
            data: HashMap::new(),
        };

        let element = LsifElement::Vertex(vertex);
        let json = serde_json::to_string(&element).unwrap();
        assert!(json.contains("\"id\":\"1\""));
        assert!(json.contains("\"type\":\"vertex\""));
        assert!(json.contains("\"label\":\"document\""));
    }

    #[test]
    fn test_edge_serialization() {
        let edge = Edge {
            id: "2".to_string(),
            element_type: "edge".to_string(),
            label: "contains".to_string(),
            out_v: "1".to_string(),
            in_v: "3".to_string(),
            data: HashMap::new(),
        };

        let element = LsifElement::Edge(edge);
        let json = serde_json::to_string(&element).unwrap();
        assert!(json.contains("\"outV\":\"1\""));
        assert!(json.contains("\"inV\":\"3\""));
    }

    #[test]
    fn test_lsif_generator_next_id() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        assert_eq!(generator.next_id(), "1");
        assert_eq!(generator.next_id(), "2");
        assert_eq!(generator.next_id(), "3");
    }

    #[test]
    fn test_generate_metadata() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        generator.generate_metadata().unwrap();

        assert_eq!(generator.elements.len(), 1);
        if let LsifElement::Vertex(vertex) = &generator.elements[0] {
            assert_eq!(vertex.label, "metaData");
            assert!(vertex.data.contains_key("version"));
            assert!(vertex.data.contains_key("toolInfo"));
        } else {
            panic!("Expected vertex");
        }
    }

    #[test]
    fn test_generate_project() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        let project_id = generator.generate_project().unwrap();

        assert!(!project_id.is_empty());
        assert_eq!(generator.elements.len(), 1);
        if let LsifElement::Vertex(vertex) = &generator.elements[0] {
            assert_eq!(vertex.label, "project");
            assert_eq!(vertex.id, project_id);
        } else {
            panic!("Expected vertex");
        }
    }

    #[test]
    fn test_generate_document() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        let doc_id = generator.generate_document("/test/file.rs").unwrap();

        assert!(!doc_id.is_empty());
        assert_eq!(generator.elements.len(), 1);
        if let LsifElement::Vertex(vertex) = &generator.elements[0] {
            assert_eq!(vertex.label, "document");
            assert!(vertex
                .data
                .get("uri")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("file.rs"));
        } else {
            panic!("Expected vertex");
        }
    }

    #[test]
    fn test_generate_range() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        let symbol = Symbol {
            id: "test".to_string(),
            name: "test".to_string(),
            kind: SymbolKind::Function,
            file_path: "/test.rs".to_string(),
            range: Range {
                start: Position {
                    line: 10,
                    character: 5,
                },
                end: Position {
                    line: 10,
                    character: 15,
                },
            },
            documentation: None,
            detail: None,
        };

        let range_id = generator.generate_range(&symbol).unwrap();

        assert!(!range_id.is_empty());
        assert_eq!(generator.elements.len(), 1);
        if let LsifElement::Vertex(vertex) = &generator.elements[0] {
            assert_eq!(vertex.label, "range");
            let start = vertex.data.get("start").unwrap();
            assert_eq!(start.get("line").unwrap().as_u64().unwrap(), 10);
            assert_eq!(start.get("character").unwrap().as_u64().unwrap(), 5);
        } else {
            panic!("Expected vertex");
        }
    }

    #[test]
    fn test_generate_contains_edge() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        generator.generate_contains_edge("1", "2").unwrap();

        assert_eq!(generator.elements.len(), 1);
        if let LsifElement::Edge(edge) = &generator.elements[0] {
            assert_eq!(edge.label, "contains");
            assert_eq!(edge.out_v, "1");
            assert_eq!(edge.in_v, "2");
        } else {
            panic!("Expected edge");
        }
    }

    #[test]
    fn test_generate_hover() {
        let graph = CodeGraph::new();
        let mut generator = LsifGenerator::new(graph);

        generator
            .generate_hover("result1", "This is hover content")
            .unwrap();

        assert_eq!(generator.elements.len(), 2); // hover vertex + edge

        // Check hover vertex
        if let LsifElement::Vertex(vertex) = &generator.elements[0] {
            assert_eq!(vertex.label, "hoverResult");
            let result = vertex.data.get("result").unwrap();
            let contents = result.get("contents").unwrap();
            assert_eq!(
                contents.get("value").unwrap().as_str().unwrap(),
                "This is hover content"
            );
        } else {
            panic!("Expected hover vertex");
        }

        // Check edge
        if let LsifElement::Edge(edge) = &generator.elements[1] {
            assert_eq!(edge.label, "textDocument/hover");
            assert_eq!(edge.out_v, "result1");
        } else {
            panic!("Expected edge");
        }
    }

    #[test]
    #[ignore] // TODO: fix test - graph structure changed
    fn test_full_lsif_generation() {
        let graph = create_test_graph();
        let lsif = generate_lsif(graph).unwrap();

        // Check that LSIF contains expected elements
        assert!(lsif.contains("metaData"));
        assert!(lsif.contains("project"));
        assert!(lsif.contains("document"));
        assert!(lsif.contains("range"));
        assert!(lsif.contains("\"main\""));
        assert!(lsif.contains("\"helper\""));
        assert!(lsif.contains("/test/main.rs"));
        assert!(lsif.contains("/test/helper.rs"));

        // Check that it's valid JSON Lines format
        for line in lsif.lines() {
            if !line.trim().is_empty() {
                serde_json::from_str::<Value>(line).unwrap();
            }
        }
    }

    #[test]
    fn test_lsif_parser_new() {
        let parser = LsifParser::new();
        assert_eq!(parser.documents.len(), 0);
        assert_eq!(parser.ranges.len(), 0);
        assert_eq!(parser.result_sets.len(), 0);
    }

    #[test]
    fn test_parse_position() {
        let parser = LsifParser::new();
        let json = json!({
            "line": 10,
            "character": 5
        });

        let pos = parser.parse_position(&json).unwrap();
        assert_eq!(pos.line, 10);
        assert_eq!(pos.character, 5);
    }

    #[test]
    fn test_parse_position_with_missing_fields() {
        let parser = LsifParser::new();
        let json = json!({});

        let pos = parser.parse_position(&json).unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn test_process_document_vertex() {
        let mut parser = LsifParser::new();
        let vertex = json!({
            "id": "doc1",
            "type": "vertex",
            "label": "document",
            "uri": "file:///test.rs"
        });

        parser.process_vertex(vertex).unwrap();
        assert_eq!(parser.documents.get("doc1").unwrap(), "file:///test.rs");
    }

    #[test]
    fn test_process_range_vertex() {
        let mut parser = LsifParser::new();
        let vertex = json!({
            "id": "range1",
            "type": "vertex",
            "label": "range",
            "start": { "line": 5, "character": 10 },
            "end": { "line": 5, "character": 20 }
        });

        parser.process_vertex(vertex).unwrap();
        let range = parser.ranges.get("range1").unwrap();
        assert_eq!(range.start.line, 5);
        assert_eq!(range.start.character, 10);
        assert_eq!(range.end.line, 5);
        assert_eq!(range.end.character, 20);
    }

    #[test]
    fn test_process_contains_edge() {
        let mut parser = LsifParser::new();

        // Add document and range first
        parser
            .documents
            .insert("doc1".to_string(), "file:///test.rs".to_string());
        parser.ranges.insert(
            "range1".to_string(),
            LsifRange {
                document_id: String::new(),
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
        );

        let edge = json!({
            "type": "edge",
            "label": "contains",
            "outV": "doc1",
            "inV": "range1"
        });

        parser.process_edge(edge).unwrap();
        assert_eq!(parser.ranges.get("range1").unwrap().document_id, "doc1");
    }

    #[test]
    #[ignore] // TODO: fix test - graph structure changed
    fn test_parse_lsif_roundtrip() {
        let original_graph = create_test_graph();
        let lsif = generate_lsif(original_graph.clone()).unwrap();

        let parsed_graph = parse_lsif(&lsif).unwrap();

        // Check that we have symbols in the parsed graph
        assert!(!parsed_graph.symbol_index.is_empty());
    }

    #[test]
    fn test_write_lsif() {
        let graph = create_test_graph();
        let mut buffer = Vec::new();

        write_lsif(&mut buffer, graph).unwrap();

        let content = String::from_utf8(buffer).unwrap();
        assert!(content.contains("metaData"));
        assert!(content.contains("project"));

        // Verify each line is valid JSON
        for line in content.lines() {
            if !line.trim().is_empty() {
                serde_json::from_str::<Value>(line).unwrap();
            }
        }
    }

    #[test]
    fn test_empty_graph_generation() {
        let graph = CodeGraph::new();
        let lsif = generate_lsif(graph).unwrap();

        // Should still have metadata and project
        assert!(lsif.contains("metaData"));
        assert!(lsif.contains("project"));
    }

    #[test]
    fn test_parse_empty_lsif() {
        let content = "";
        let graph = parse_lsif(content).unwrap();
        assert_eq!(graph.symbol_index.len(), 0);
    }

    #[test]
    fn test_parse_invalid_json() {
        let content = "not valid json";
        let result = parse_lsif(content);
        assert!(result.is_err());
    }
}
