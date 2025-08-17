pub mod core;
pub mod cli;

// Re-export commonly used types from core
pub use core::{
    CodeGraph, Symbol, SymbolKind, Range, Position, EdgeKind,
    generate_lsif, parse_lsif, write_lsif,
};

// Re-export IO/CLI components
pub use cli::{
    Cli,
    storage::{IndexStorage, IndexMetadata, IndexFormat},
    lsp_client::LspClient,
    lsp_indexer::LspIndexer,
};