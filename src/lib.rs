pub mod cli;
pub mod core;

// Re-export commonly used types from core
pub use core::{
    generate_lsif, parse_lsif, write_lsif, CodeGraph, EdgeKind, Position, Range, Symbol, SymbolKind,
};

// Re-export IO/CLI components
pub use cli::{
    lsp_client::LspClient,
    lsp_indexer::LspIndexer,
    storage::{IndexFormat, IndexMetadata, IndexStorage},
    Cli,
};
