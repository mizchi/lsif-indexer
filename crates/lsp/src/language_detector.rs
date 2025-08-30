use crate::go_adapter::GoAdapter;
use crate::minimal_language_adapter::MinimalLanguageAdapter;
use crate::python_adapter::PythonAdapter;
use crate::typescript_adapter::TypeScriptAdapter;
/// 言語自動検出とアダプタ選択モジュール
use std::path::Path;

/// サポートされている言語
#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    Go,
    Python,
    TypeScript,
    JavaScript,
    Unknown,
}

impl Language {
    /// 文字列から言語を判定
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Language::Rust,
            "go" | "golang" => Language::Go,
            "python" | "py" => Language::Python,
            "typescript" | "ts" => Language::TypeScript,
            "javascript" | "js" => Language::JavaScript,
            _ => Language::Unknown,
        }
    }

    /// 言語の表示名
    pub fn name(&self) -> &str {
        match self {
            Language::Rust => "Rust",
            Language::Go => "Go",
            Language::Python => "Python",
            Language::TypeScript => "TypeScript",
            Language::JavaScript => "JavaScript",
            Language::Unknown => "Unknown",
        }
    }

    /// 言語の拡張子リスト
    pub fn extensions(&self) -> Vec<&str> {
        match self {
            Language::Rust => vec!["rs"],
            Language::Go => vec!["go"],
            Language::Python => vec!["py", "pyi"],
            Language::TypeScript => vec!["ts", "tsx"],
            Language::JavaScript => vec!["js", "jsx", "mjs", "cjs"],
            Language::Unknown => vec![],
        }
    }
}

/// プロジェクトの主要言語を検出
pub fn detect_project_language(project_path: &Path) -> Language {
    // プロジェクトファイルから言語を判定
    if project_path.join("Cargo.toml").exists() {
        return Language::Rust;
    }
    if project_path.join("go.mod").exists() {
        return Language::Go;
    }
    if project_path.join("requirements.txt").exists()
        || project_path.join("setup.py").exists()
        || project_path.join("pyproject.toml").exists()
    {
        return Language::Python;
    }
    if project_path.join("package.json").exists() {
        // package.jsonの内容を確認してTypeScriptかJavaScriptか判定
        if project_path.join("tsconfig.json").exists() {
            return Language::TypeScript;
        }
        return Language::JavaScript;
    }

    // ファイル拡張子から判定
    let mut file_counts = std::collections::HashMap::new();

    if let Ok(entries) = std::fs::read_dir(project_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if let Some(ext_str) = ext.to_str() {
                        *file_counts.entry(ext_str.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // 最も多い拡張子から言語を判定
    for lang in &[
        Language::Rust,
        Language::Go,
        Language::Python,
        Language::TypeScript,
        Language::JavaScript,
    ] {
        for ext in lang.extensions() {
            if file_counts.get(ext).copied().unwrap_or(0) > 0 {
                return lang.clone();
            }
        }
    }

    Language::Unknown
}

/// 言語に応じたLSPアダプタを作成
pub fn create_language_adapter(language: &Language) -> Option<Box<dyn MinimalLanguageAdapter>> {
    match language {
        Language::Go => Some(Box::new(GoAdapter)),
        Language::Python => Some(Box::new(PythonAdapter::new())),
        Language::TypeScript => Some(Box::new(TypeScriptAdapter::new())),
        Language::JavaScript => Some(Box::new(TypeScriptAdapter::javascript_only())),
        Language::Rust | Language::Unknown => None, // RustはLSP未実装、Unknownはサポート外
    }
}

/// ファイルから言語を判定
pub fn detect_file_language(file_path: &Path) -> Language {
    if let Some(ext) = file_path.extension() {
        if let Some(ext_str) = ext.to_str() {
            match ext_str {
                "rs" => return Language::Rust,
                "go" => return Language::Go,
                "py" | "pyi" => return Language::Python,
                "ts" | "tsx" => return Language::TypeScript,
                "js" | "jsx" | "mjs" | "cjs" => return Language::JavaScript,
                _ => {}
            }
        }
    }
    Language::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_string("rust"), Language::Rust);
        assert_eq!(Language::from_string("go"), Language::Go);
        assert_eq!(Language::from_string("python"), Language::Python);
        assert_eq!(Language::from_string("typescript"), Language::TypeScript);
        assert_eq!(Language::from_string("javascript"), Language::JavaScript);
        assert_eq!(Language::from_string("unknown"), Language::Unknown);
    }

    #[test]
    fn test_detect_rust_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("Cargo.toml"), "[package]").unwrap();

        assert_eq!(detect_project_language(temp_dir.path()), Language::Rust);
    }

    #[test]
    fn test_detect_go_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("go.mod"), "module test").unwrap();

        assert_eq!(detect_project_language(temp_dir.path()), Language::Go);
    }

    #[test]
    fn test_detect_python_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("requirements.txt"), "flask==2.0").unwrap();

        assert_eq!(detect_project_language(temp_dir.path()), Language::Python);
    }

    #[test]
    fn test_detect_typescript_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package.json"), "{}").unwrap();
        fs::write(temp_dir.path().join("tsconfig.json"), "{}").unwrap();

        assert_eq!(
            detect_project_language(temp_dir.path()),
            Language::TypeScript
        );
    }

    #[test]
    fn test_detect_file_language() {
        assert_eq!(detect_file_language(Path::new("test.rs")), Language::Rust);
        assert_eq!(detect_file_language(Path::new("main.go")), Language::Go);
        assert_eq!(detect_file_language(Path::new("app.py")), Language::Python);
        assert_eq!(
            detect_file_language(Path::new("index.ts")),
            Language::TypeScript
        );
        assert_eq!(
            detect_file_language(Path::new("script.js")),
            Language::JavaScript
        );
    }
}
