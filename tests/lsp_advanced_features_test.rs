use anyhow::Result;
use cli::{
    go_adapter::GoAdapter, lsp_minimal_client::MinimalLspClient, python_adapter::PythonAdapter,
    typescript_adapter::TypeScriptAdapter,
};
use lsp_types::Position;
use std::fs;
use tempfile::TempDir;

/// テスト用のGoプロジェクトを作成
fn create_test_go_project() -> Result<TempDir> {
    let dir = tempfile::tempdir()?;

    // main.go
    fs::write(
        dir.path().join("main.go"),
        r#"package main

import "fmt"

type User struct {
    Name string
    Age  int
}

func (u *User) Greet() {
    fmt.Printf("Hello, I'm %s\n", u.Name)
}

func main() {
    user := &User{Name: "Alice", Age: 30}
    user.Greet()
}
"#,
    )?;

    Ok(dir)
}

/// テスト用のPythonプロジェクトを作成
fn create_test_python_project() -> Result<TempDir> {
    let dir = tempfile::tempdir()?;

    // main.py
    fs::write(
        dir.path().join("main.py"),
        r#"from typing import Optional

class User:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age
    
    def greet(self) -> None:
        print(f"Hello, I'm {self.name}")

def main() -> None:
    user = User("Alice", 30)
    user.greet()

if __name__ == "__main__":
    main()
"#,
    )?;

    Ok(dir)
}

/// テスト用のTypeScriptプロジェクトを作成
fn create_test_typescript_project() -> Result<TempDir> {
    let dir = tempfile::tempdir()?;

    // main.ts
    fs::write(
        dir.path().join("main.ts"),
        r#"interface User {
    name: string;
    age: number;
}

class UserImpl implements User {
    constructor(public name: string, public age: number) {}
    
    greet(): void {
        console.log(`Hello, I'm ${this.name}`);
    }
}

function main(): void {
    const user = new UserImpl("Alice", 30);
    user.greet();
}

main();
"#,
    )?;

    // tsconfig.json
    fs::write(
        dir.path().join("tsconfig.json"),
        r#"{
    "compilerOptions": {
        "target": "ES2020",
        "module": "commonjs",
        "strict": true
    }
}"#,
    )?;

    Ok(dir)
}

#[test]
#[ignore] // LSPサーバーがインストールされていない環境ではスキップ
fn test_go_definition() -> Result<()> {
    let dir = create_test_go_project()?;
    let adapter = Box::new(GoAdapter);
    let mut client = MinimalLspClient::new(adapter)?;

    // 初期化
    client.initialize(dir.path())?;

    // user.Greet()の参照位置から定義へジャンプ
    let main_file = dir.path().join("main.go");
    let definition = client.go_to_definition(
        &main_file,
        Position::new(15, 9), // user.Greet()のGreet部分
    )?;

    assert!(definition.is_some());
    if let Some(loc) = definition {
        assert_eq!(loc.range.start.line, 9); // Greetメソッドの定義行
    }

    client.shutdown()?;
    Ok(())
}

#[test]
#[ignore] // LSPサーバーがインストールされていない環境ではスキップ
fn test_python_hover() -> Result<()> {
    let dir = create_test_python_project()?;
    let adapter = Box::new(PythonAdapter::new());
    let mut client = MinimalLspClient::new(adapter)?;

    // 初期化
    client.initialize(dir.path())?;

    // Userクラスのホバー情報を取得
    let main_file = dir.path().join("main.py");
    let hover = client.get_hover(
        &main_file,
        Position::new(12, 11), // User("Alice", 30)のUser部分
    )?;

    assert!(hover.is_some());
    if let Some(info) = hover {
        // 型情報が含まれていることを確認
        assert!(info.contains("User") || info.contains("class"));
    }

    client.shutdown()?;
    Ok(())
}

#[test]
#[ignore] // LSPサーバーがインストールされていない環境ではスキップ
fn test_typescript_definition_and_hover() -> Result<()> {
    let dir = create_test_typescript_project()?;
    let adapter = Box::new(TypeScriptAdapter::new());
    let mut client = MinimalLspClient::new(adapter)?;

    // 初期化
    client.initialize(dir.path())?;

    let main_file = dir.path().join("main.ts");

    // UserImplの参照位置から定義へジャンプ
    let definition = client.go_to_definition(
        &main_file,
        Position::new(15, 20), // new UserImpl()のUserImpl部分
    )?;

    assert!(definition.is_some());
    if let Some(loc) = definition {
        assert_eq!(loc.range.start.line, 5); // UserImplクラスの定義行
    }

    // user変数のホバー情報を取得
    let hover = client.get_hover(
        &main_file,
        Position::new(16, 4), // user.greet()のuser部分
    )?;

    assert!(hover.is_some());
    if let Some(info) = hover {
        // UserImpl型であることが含まれているはず
        assert!(info.contains("UserImpl") || info.contains("User"));
    }

    client.shutdown()?;
    Ok(())
}

#[test]
#[ignore] // LSPサーバーがインストールされていない環境ではスキップ
fn test_go_hover_type_info() -> Result<()> {
    let dir = create_test_go_project()?;
    let adapter = Box::new(GoAdapter);
    let mut client = MinimalLspClient::new(adapter)?;

    // 初期化
    client.initialize(dir.path())?;

    // user変数の型情報を取得
    let main_file = dir.path().join("main.go");
    let hover = client.get_hover(
        &main_file,
        Position::new(14, 4), // user := &User{...}のuser部分
    )?;

    assert!(hover.is_some());
    if let Some(info) = hover {
        // *User型であることが含まれているはず
        assert!(info.contains("User") || info.contains("*User"));
    }

    client.shutdown()?;
    Ok(())
}
