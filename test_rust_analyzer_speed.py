#!/usr/bin/env python3
"""
Test rust-analyzer workspace/symbol performance directly
"""

import json
import subprocess
import time
import sys
from pathlib import Path

def send_lsp_message(proc, message):
    """Send a message to the LSP server"""
    content = json.dumps(message)
    header = f"Content-Length: {len(content)}\r\n\r\n"
    full_message = header + content
    proc.stdin.write(full_message.encode())
    proc.stdin.flush()

def read_lsp_message(proc):
    """Read a message from the LSP server"""
    # Read header
    header = b""
    while not header.endswith(b"\r\n\r\n"):
        header += proc.stdout.read(1)
    
    # Parse content length
    content_length = 0
    for line in header.decode().split("\r\n"):
        if line.startswith("Content-Length:"):
            content_length = int(line.split(":")[1].strip())
            break
    
    # Read content
    content = proc.stdout.read(content_length)
    return json.loads(content.decode())

def test_rust_analyzer(project_path):
    """Test rust-analyzer performance on a project"""
    print(f"Testing rust-analyzer on: {project_path}")
    
    # Start rust-analyzer
    proc = subprocess.Popen(
        ["rust-analyzer"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=project_path
    )
    
    try:
        # Initialize
        print("Initializing rust-analyzer...")
        init_start = time.time()
        
        initialize_params = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": None,
                "rootUri": f"file://{Path(project_path).absolute()}",
                "capabilities": {
                    "workspace": {
                        "symbol": {
                            "dynamicRegistration": False
                        }
                    }
                },
                "initializationOptions": {
                    "cargo": {
                        "buildScripts": {
                            "enable": False
                        }
                    },
                    "procMacro": {
                        "enable": False
                    }
                }
            }
        }
        
        send_lsp_message(proc, initialize_params)
        response = read_lsp_message(proc)
        init_time = time.time() - init_start
        print(f"Initialization took: {init_time:.2f}s")
        
        # Send initialized notification
        initialized = {
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }
        send_lsp_message(proc, initialized)
        
        # Wait a bit for indexing
        print("Waiting for initial indexing...")
        time.sleep(2)
        
        # Test workspace/symbol
        print("\nTesting workspace/symbol...")
        queries = ["", "main", "test", "fn", "struct"]
        
        for query in queries:
            symbol_start = time.time()
            
            workspace_symbol = {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "workspace/symbol",
                "params": {
                    "query": query
                }
            }
            
            send_lsp_message(proc, workspace_symbol)
            response = read_lsp_message(proc)
            symbol_time = time.time() - symbol_start
            
            if "result" in response:
                symbol_count = len(response["result"])
                print(f"Query '{query}': {symbol_count} symbols in {symbol_time:.3f}s")
            else:
                print(f"Query '{query}': Error - {response.get('error', 'Unknown error')}")
        
        # Test document/symbol on specific files
        print("\nTesting document/symbol on individual files...")
        
        # Find some .rs files
        rs_files = list(Path(project_path).glob("**/*.rs"))[:5]  # Test first 5 files
        
        total_doc_time = 0
        for rs_file in rs_files:
            doc_start = time.time()
            
            # First open the document
            did_open = {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": f"file://{rs_file.absolute()}",
                        "languageId": "rust",
                        "version": 1,
                        "text": rs_file.read_text()
                    }
                }
            }
            send_lsp_message(proc, did_open)
            
            # Request symbols
            doc_symbol = {
                "jsonrpc": "2.0",
                "id": 100 + rs_files.index(rs_file),
                "method": "textDocument/documentSymbol",
                "params": {
                    "textDocument": {
                        "uri": f"file://{rs_file.absolute()}"
                    }
                }
            }
            
            send_lsp_message(proc, doc_symbol)
            response = read_lsp_message(proc)
            doc_time = time.time() - doc_start
            total_doc_time += doc_time
            
            if "result" in response:
                symbol_count = len(response["result"]) if response["result"] else 0
                print(f"  {rs_file.name}: {symbol_count} symbols in {doc_time:.3f}s")
            else:
                print(f"  {rs_file.name}: Error")
        
        if rs_files:
            print(f"\nAverage document/symbol time: {total_doc_time/len(rs_files):.3f}s")
        
        # Shutdown
        shutdown = {
            "jsonrpc": "2.0",
            "id": 999,
            "method": "shutdown"
        }
        send_lsp_message(proc, shutdown)
        
    finally:
        proc.terminate()
        proc.wait(timeout=5)

if __name__ == "__main__":
    # Test on current project
    test_rust_analyzer(".")
    
    # Test on test project if it exists
    test_path = "/tmp/test-lsif-large"
    if Path(test_path).exists():
        print("\n" + "="*50 + "\n")
        test_rust_analyzer(test_path)