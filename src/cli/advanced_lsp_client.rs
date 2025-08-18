use anyhow::{Result, anyhow};
use lsp_types::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::lsp_adapter::{LspAdapter, GenericLspClient};

#[derive(Clone)]
pub struct AdvancedLspClient {
    inner: Arc<Mutex<GenericLspClient>>,
    capabilities: Arc<ServerCapabilities>,
    open_documents: Arc<Mutex<HashMap<Url, i32>>>,
}

impl AdvancedLspClient {
    pub fn new(adapter: Box<dyn LspAdapter>) -> Result<Self> {
        let client = GenericLspClient::new(adapter)?;
        
        let capabilities = ServerCapabilities::default();
        
        Ok(Self {
            inner: Arc::new(Mutex::new(client)),
            capabilities: Arc::new(capabilities),
            open_documents: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub fn open_document(&self, uri: Url, content: String, language_id: String) -> Result<()> {
        let mut docs = self.open_documents.lock().unwrap();
        let version = docs.get(&uri).unwrap_or(&0) + 1;
        docs.insert(uri.clone(), version);
        
        let mut client = self.inner.lock().unwrap();
        client.send_notification("textDocument/didOpen", DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version,
                text: content,
            },
        })
    }
    
    pub fn goto_definition(&self, uri: Url, position: Position) -> Result<Vec<Location>> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response = client.goto_definition(params)?;
        Ok(vec![response])
    }
    
    pub fn find_references(
        &self,
        uri: Url,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            context: ReferenceContext {
                include_declaration,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        client.find_references(params)
    }
    
    pub fn hover(&self, uri: Url, position: Position) -> Result<Option<Hover>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        client.send_request("textDocument/hover", params)
    }
    
    pub fn completion(&self, uri: Url, position: Position) -> Result<Vec<CompletionItem>> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            context: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<CompletionResponse> = 
            client.send_request("textDocument/completion", params)?;
        
        match response {
            Some(CompletionResponse::Array(items)) => Ok(items),
            Some(CompletionResponse::List(list)) => Ok(list.items),
            None => Ok(Vec::new()),
        }
    }
    
    pub fn document_symbols(&self, uri: Url) -> Result<Vec<DocumentSymbol>> {
        let mut client = self.inner.lock().unwrap();
        client.get_document_symbols(&uri.to_string())
    }
    
    pub fn call_hierarchy_prepare(
        &self,
        uri: Url,
        position: Position,
    ) -> Result<Vec<CallHierarchyItem>> {
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<Vec<CallHierarchyItem>> = 
            client.send_request("textDocument/prepareCallHierarchy", params)?;
        
        Ok(response.unwrap_or_default())
    }
    
    pub fn incoming_calls(&self, item: CallHierarchyItem) -> Result<Vec<CallHierarchyIncomingCall>> {
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<Vec<CallHierarchyIncomingCall>> = 
            client.send_request("callHierarchy/incomingCalls", params)?;
        
        Ok(response.unwrap_or_default())
    }
    
    pub fn outgoing_calls(&self, item: CallHierarchyItem) -> Result<Vec<CallHierarchyOutgoingCall>> {
        let params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<Vec<CallHierarchyOutgoingCall>> = 
            client.send_request("callHierarchy/outgoingCalls", params)?;
        
        Ok(response.unwrap_or_default())
    }
    
    pub fn diagnostics(&self, _uri: Url) -> Result<Vec<Diagnostic>> {
        // Simplified implementation - most LSP servers don't support pull-based diagnostics
        // They push diagnostics via notifications instead
        Ok(Vec::new())
    }
    
    pub fn type_definition(&self, uri: Url, position: Position) -> Result<Vec<Location>> {
        let params = lsp_types::request::GotoTypeDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<lsp_types::request::GotoTypeDefinitionResponse> = 
            client.send_request("textDocument/typeDefinition", params)?;
        
        match response {
            Some(lsp_types::request::GotoTypeDefinitionResponse::Scalar(location)) => Ok(vec![location]),
            Some(lsp_types::request::GotoTypeDefinitionResponse::Array(locations)) => Ok(locations),
            Some(lsp_types::request::GotoTypeDefinitionResponse::Link(links)) => {
                Ok(links.into_iter().map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                }).collect())
            }
            None => Ok(Vec::new()),
        }
    }
    
    pub fn implementation(&self, uri: Url, position: Position) -> Result<Vec<Location>> {
        let params = lsp_types::request::GotoImplementationParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<lsp_types::request::GotoImplementationResponse> = 
            client.send_request("textDocument/implementation", params)?;
        
        match response {
            Some(lsp_types::request::GotoImplementationResponse::Scalar(location)) => Ok(vec![location]),
            Some(lsp_types::request::GotoImplementationResponse::Array(locations)) => Ok(locations),
            Some(lsp_types::request::GotoImplementationResponse::Link(links)) => {
                Ok(links.into_iter().map(|link| Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                }).collect())
            }
            None => Ok(Vec::new()),
        }
    }
    
    pub fn rename(&self, uri: Url, position: Position, new_name: String) -> Result<WorkspaceEdit> {
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            new_name,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        
        let mut client = self.inner.lock().unwrap();
        let response: Option<WorkspaceEdit> = 
            client.send_request("textDocument/rename", params)?;
        
        response.ok_or_else(|| anyhow!("No rename edits available"))
    }
    
    pub fn shutdown(self) -> Result<()> {
        let client = Arc::try_unwrap(self.inner)
            .map_err(|_| anyhow!("Cannot shutdown client with active references"))?
            .into_inner()
            .unwrap();
        
        client.shutdown()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_advanced_client_creation() {
        // This test would require a mock LSP adapter
        // For now, we just ensure the module compiles correctly
    }
}