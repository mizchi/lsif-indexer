//! テストフィクスチャ - test_projectsの代替として各言語のサンプルコードを提供

use std::collections::HashMap;

/// 各言語のテストコードサンプル
pub struct TestFixtures;

impl TestFixtures {
    /// すべての言語のサンプルを取得
    pub fn all_samples() -> HashMap<String, Vec<TestFile>> {
        let mut samples = HashMap::new();
        samples.insert("rust".to_string(), Self::rust_samples());
        samples.insert("typescript".to_string(), Self::typescript_samples());
        samples.insert("go".to_string(), Self::go_samples());
        samples.insert("python".to_string(), Self::python_samples());
        samples
    }

    /// Rustのサンプルコード
    pub fn rust_samples() -> Vec<TestFile> {
        vec![
            TestFile {
                path: "src/main.rs".to_string(),
                content: include_str!("fixtures/rust/main.rs").to_string(),
            },
            TestFile {
                path: "src/lib.rs".to_string(),
                content: include_str!("fixtures/rust/lib.rs").to_string(),
            },
            TestFile {
                path: "Cargo.toml".to_string(),
                content: include_str!("fixtures/rust/Cargo.toml").to_string(),
            },
        ]
    }

    /// TypeScriptのサンプルコード
    pub fn typescript_samples() -> Vec<TestFile> {
        vec![
            TestFile {
                path: "index.ts".to_string(),
                content: include_str!("fixtures/typescript/index.ts").to_string(),
            },
            TestFile {
                path: "utils.ts".to_string(),
                content: include_str!("fixtures/typescript/utils.ts").to_string(),
            },
            TestFile {
                path: "tsconfig.json".to_string(),
                content: include_str!("fixtures/typescript/tsconfig.json").to_string(),
            },
        ]
    }

    /// Goのサンプルコード
    pub fn go_samples() -> Vec<TestFile> {
        vec![
            TestFile {
                path: "main.go".to_string(),
                content: include_str!("fixtures/go/main.go").to_string(),
            },
            TestFile {
                path: "utils.go".to_string(),
                content: include_str!("fixtures/go/utils.go").to_string(),
            },
            TestFile {
                path: "go.mod".to_string(),
                content: include_str!("fixtures/go/go.mod").to_string(),
            },
        ]
    }

    /// Pythonのサンプルコード
    pub fn python_samples() -> Vec<TestFile> {
        vec![
            TestFile {
                path: "main.py".to_string(),
                content: include_str!("fixtures/python/main.py").to_string(),
            },
            TestFile {
                path: "utils.py".to_string(),
                content: include_str!("fixtures/python/utils.py").to_string(),
            },
            TestFile {
                path: "__init__.py".to_string(),
                content: "".to_string(),
            },
        ]
    }
}

/// テストファイル
#[derive(Clone, Debug)]
pub struct TestFile {
    pub path: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_samples_available() {
        let samples = TestFixtures::all_samples();

        // 4言語分のサンプルが存在
        assert_eq!(samples.len(), 4);

        // 各言語に最低2ファイル以上
        for (lang, files) in samples {
            assert!(files.len() >= 2, "{} has less than 2 files", lang);

            // 各ファイルにコンテンツが存在
            for file in files {
                assert!(
                    !file.content.is_empty() || file.path.contains("__init__"),
                    "{}/{} is empty",
                    lang,
                    file.path
                );
            }
        }
    }
}
