use cli::differential_indexer::DifferentialIndexer;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// 現実的なプロジェクト構造を作成（30-50ファイル）
fn create_realistic_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // src/lib.rs - メインライブラリ
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    fs::write(
        src_dir.join("lib.rs"),
        r#"
//! Main library module
pub mod config;
pub mod database;
pub mod handlers;
pub mod models;
pub mod utils;

pub use config::Config;
pub use database::Database;

pub fn initialize() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
"#,
    )
    .unwrap();

    // src/config.rs
    fs::write(
        src_dir.join("config.rs"),
        r#"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub workers: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: String::from("postgresql://localhost/mydb"),
            port: 8080,
            workers: 4,
        }
    }
}
"#,
    )
    .unwrap();

    // src/database/mod.rs
    let db_dir = src_dir.join("database");
    fs::create_dir_all(&db_dir).unwrap();

    fs::write(
        db_dir.join("mod.rs"),
        r#"
pub mod connection;
pub mod models;
pub mod queries;

pub struct Database {
    connection: connection::Connection,
}

impl Database {
    pub fn new(url: &str) -> Self {
        Self {
            connection: connection::Connection::new(url),
        }
    }
}
"#,
    )
    .unwrap();

    // src/database/connection.rs
    fs::write(
        db_dir.join("connection.rs"),
        r#"
pub struct Connection {
    url: String,
}

impl Connection {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }
    
    pub fn execute(&self, query: &str) -> Result<(), String> {
        Ok(())
    }
}
"#,
    )
    .unwrap();

    // src/database/models.rs
    fs::write(
        db_dir.join("models.rs"),
        r#"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub content: String,
}
"#,
    )
    .unwrap();

    // src/database/queries.rs
    fs::write(
        db_dir.join("queries.rs"),
        r#"
use super::models::{User, Post};

pub fn find_user_by_id(id: i32) -> Option<User> {
    None
}

pub fn find_posts_by_user(user_id: i32) -> Vec<Post> {
    vec![]
}
"#,
    )
    .unwrap();

    // src/handlers/mod.rs
    let handlers_dir = src_dir.join("handlers");
    fs::create_dir_all(&handlers_dir).unwrap();

    fs::write(
        handlers_dir.join("mod.rs"),
        r#"
pub mod auth;
pub mod users;
pub mod posts;
pub mod api;
"#,
    )
    .unwrap();

    // src/handlers/auth.rs
    fs::write(
        handlers_dir.join("auth.rs"),
        r#"
pub fn login(username: &str, password: &str) -> Result<String, String> {
    Ok("token".to_string())
}

pub fn logout(token: &str) -> Result<(), String> {
    Ok(())
}

pub fn verify_token(token: &str) -> bool {
    true
}
"#,
    )
    .unwrap();

    // src/handlers/users.rs
    fs::write(
        handlers_dir.join("users.rs"),
        r#"
use crate::database::models::User;

pub fn get_user(id: i32) -> Option<User> {
    None
}

pub fn create_user(name: &str, email: &str) -> User {
    User {
        id: 1,
        name: name.to_string(),
        email: email.to_string(),
    }
}

pub fn update_user(id: i32, name: &str, email: &str) -> Result<User, String> {
    Ok(User {
        id,
        name: name.to_string(),
        email: email.to_string(),
    })
}
"#,
    )
    .unwrap();

    // src/handlers/posts.rs
    fs::write(
        handlers_dir.join("posts.rs"),
        r#"
use crate::database::models::Post;

pub fn get_post(id: i32) -> Option<Post> {
    None
}

pub fn create_post(user_id: i32, title: &str, content: &str) -> Post {
    Post {
        id: 1,
        user_id,
        title: title.to_string(),
        content: content.to_string(),
    }
}

pub fn list_posts(limit: usize) -> Vec<Post> {
    vec![]
}
"#,
    )
    .unwrap();

    // src/handlers/api.rs
    fs::write(
        handlers_dir.join("api.rs"),
        r#"
pub fn health_check() -> &'static str {
    "OK"
}

pub fn version() -> &'static str {
    "1.0.0"
}
"#,
    )
    .unwrap();

    // src/models/mod.rs
    let models_dir = src_dir.join("models");
    fs::create_dir_all(&models_dir).unwrap();

    fs::write(
        models_dir.join("mod.rs"),
        r#"
pub mod request;
pub mod response;
pub mod errors;
"#,
    )
    .unwrap();

    // src/models/request.rs
    fs::write(
        models_dir.join("request.rs"),
        r#"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
}
"#,
    )
    .unwrap();

    // src/models/response.rs
    fs::write(
        models_dir.join("response.rs"),
        r#"
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}
"#,
    )
    .unwrap();

    // src/models/errors.rs
    fs::write(
        models_dir.join("errors.rs"),
        r#"
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound,
    Unauthorized,
    BadRequest(String),
    InternalError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::NotFound => write!(f, "Not found"),
            AppError::Unauthorized => write!(f, "Unauthorized"),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}
"#,
    )
    .unwrap();

    // src/utils/mod.rs
    let utils_dir = src_dir.join("utils");
    fs::create_dir_all(&utils_dir).unwrap();

    fs::write(
        utils_dir.join("mod.rs"),
        r#"
pub mod crypto;
pub mod validation;
pub mod logger;
pub mod cache;
"#,
    )
    .unwrap();

    // src/utils/crypto.rs
    fs::write(
        utils_dir.join("crypto.rs"),
        r#"
pub fn hash_password(password: &str) -> String {
    format!("hashed_{}", password)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    hash == format!("hashed_{}", password)
}

pub fn generate_token() -> String {
    "random_token_123456".to_string()
}
"#,
    )
    .unwrap();

    // src/utils/validation.rs
    fs::write(
        utils_dir.join("validation.rs"),
        r#"
pub fn is_valid_email(email: &str) -> bool {
    email.contains('@')
}

pub fn is_valid_username(username: &str) -> bool {
    username.len() >= 3 && username.len() <= 20
}

pub fn sanitize_html(input: &str) -> String {
    input.replace('<', "&lt;").replace('>', "&gt;")
}
"#,
    )
    .unwrap();

    // src/utils/logger.rs
    fs::write(
        utils_dir.join("logger.rs"),
        r#"
pub fn info(message: &str) {
    println!("[INFO] {}", message);
}

pub fn error(message: &str) {
    eprintln!("[ERROR] {}", message);
}

pub fn debug(message: &str) {
    println!("[DEBUG] {}", message);
}
"#,
    )
    .unwrap();

    // src/utils/cache.rs
    fs::write(
        utils_dir.join("cache.rs"),
        r#"
use std::collections::HashMap;

pub struct Cache<K, V> {
    data: HashMap<K, V>,
}

impl<K: Eq + std::hash::Hash, V: Clone> Cache<K, V> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
    
    pub fn get(&self, key: &K) -> Option<V> {
        self.data.get(key).cloned()
    }
    
    pub fn set(&mut self, key: K, value: V) {
        self.data.insert(key, value);
    }
}
"#,
    )
    .unwrap();

    // tests/integration.rs
    let tests_dir = root.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();

    fs::write(
        tests_dir.join("integration.rs"),
        r#"
#[test]
fn test_database_connection() {
    assert!(true);
}

#[test]
fn test_user_creation() {
    assert!(true);
}

#[test]
fn test_authentication() {
    assert!(true);
}
"#,
    )
    .unwrap();

    // benches/benchmarks.rs
    let benches_dir = root.join("benches");
    fs::create_dir_all(&benches_dir).unwrap();

    fs::write(
        benches_dir.join("benchmarks.rs"),
        r#"
fn bench_hash_password() {
    // Benchmark implementation
}

fn bench_database_query() {
    // Benchmark implementation
}
"#,
    )
    .unwrap();

    // examples/main.rs
    let examples_dir = root.join("examples");
    fs::create_dir_all(&examples_dir).unwrap();

    fs::write(
        examples_dir.join("main.rs"),
        r#"
fn main() {
    println!("Example application");
}
"#,
    )
    .unwrap();

    // Additional service files
    let services_dir = src_dir.join("services");
    fs::create_dir_all(&services_dir).unwrap();

    // src/services/mod.rs
    fs::write(
        services_dir.join("mod.rs"),
        r#"
pub mod email;
pub mod notification;
pub mod payment;
pub mod analytics;
"#,
    )
    .unwrap();

    // src/services/email.rs
    fs::write(
        services_dir.join("email.rs"),
        r#"
pub struct EmailService {
    smtp_server: String,
}

impl EmailService {
    pub fn new(smtp_server: &str) -> Self {
        Self {
            smtp_server: smtp_server.to_string(),
        }
    }
    
    pub fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        Ok(())
    }
}
"#,
    )
    .unwrap();

    // src/services/notification.rs
    fs::write(
        services_dir.join("notification.rs"),
        r#"
pub enum NotificationType {
    Email,
    Push,
    SMS,
}

pub fn send_notification(user_id: i32, message: &str, notification_type: NotificationType) {
    // Send notification
}
"#,
    )
    .unwrap();

    // src/services/payment.rs
    fs::write(
        services_dir.join("payment.rs"),
        r#"
pub struct PaymentService {
    api_key: String,
}

impl PaymentService {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
        }
    }
    
    pub fn process_payment(&self, amount: f64, currency: &str) -> Result<String, String> {
        Ok("transaction_id_123".to_string())
    }
}
"#,
    )
    .unwrap();

    // src/services/analytics.rs
    fs::write(
        services_dir.join("analytics.rs"),
        r#"
use std::collections::HashMap;

pub struct Analytics {
    events: Vec<Event>,
}

pub struct Event {
    name: String,
    properties: HashMap<String, String>,
}

impl Analytics {
    pub fn track(&mut self, event_name: &str) {
        // Track event
    }
}
"#,
    )
    .unwrap();

    // src/middleware/mod.rs
    let middleware_dir = src_dir.join("middleware");
    fs::create_dir_all(&middleware_dir).unwrap();

    fs::write(
        middleware_dir.join("mod.rs"),
        r#"
pub mod auth;
pub mod cors;
pub mod rate_limit;
"#,
    )
    .unwrap();

    // src/middleware/auth.rs
    fs::write(
        middleware_dir.join("auth.rs"),
        r#"
pub fn verify_auth_token(token: &str) -> bool {
    !token.is_empty()
}
"#,
    )
    .unwrap();

    // src/middleware/cors.rs
    fs::write(
        middleware_dir.join("cors.rs"),
        r#"
pub struct CorsConfig {
    allowed_origins: Vec<String>,
}

impl CorsConfig {
    pub fn new() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
        }
    }
}
"#,
    )
    .unwrap();

    // src/middleware/rate_limit.rs
    fs::write(
        middleware_dir.join("rate_limit.rs"),
        r#"
use std::time::Duration;

pub struct RateLimiter {
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            max_requests,
            window,
        }
    }
    
    pub fn check(&self, client_id: &str) -> bool {
        true
    }
}
"#,
    )
    .unwrap();

    temp_dir
}

/// インデックス作成のベンチマーク
fn benchmark_index_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_creation");
    group.sample_size(10); // サンプル数を減らして実行時間を短縮

    // フォールバックオンリーモードのベンチマーク
    group.bench_function("fallback_only_30files", |b| {
        b.iter_with_setup(
            || {
                let project = create_realistic_project();
                let db_dir = project.path().join("test_db");
                (project, db_dir)
            },
            |(project, db_dir)| {
                let mut indexer =
                    DifferentialIndexer::new(db_dir.to_str().unwrap(), project.path()).unwrap();
                indexer.set_fallback_only(true);
                black_box(indexer.full_reindex().unwrap())
            },
        );
    });

    // LSPモードのベンチマーク（rust-analyzer使用）
    group.bench_function("lsp_mode_30files", |b| {
        b.iter_with_setup(
            || {
                let project = create_realistic_project();
                let db_dir = project.path().join("test_db");
                (project, db_dir)
            },
            |(project, db_dir)| {
                let mut indexer =
                    DifferentialIndexer::new(db_dir.to_str().unwrap(), project.path()).unwrap();
                indexer.set_fallback_only(false);
                black_box(indexer.full_reindex().unwrap())
            },
        );
    });

    group.finish();
}

/// 差分インデックスのベンチマーク
fn benchmark_differential_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("differential_index");
    group.sample_size(10);

    group.bench_function("single_file_change", |b| {
        b.iter_with_setup(
            || {
                let project = create_realistic_project();
                let db_dir = project.path().join("test_db");

                // 初回インデックスを作成
                let mut indexer =
                    DifferentialIndexer::new(db_dir.to_str().unwrap(), project.path()).unwrap();
                indexer.set_fallback_only(true);
                indexer.full_reindex().unwrap();

                // ファイルを変更
                let file_path = project.path().join("src/lib.rs");
                let content = fs::read_to_string(&file_path).unwrap();
                fs::write(&file_path, format!("{}\n// Modified", content)).unwrap();

                (project, db_dir)
            },
            |(project, db_dir)| {
                let mut indexer =
                    DifferentialIndexer::new(db_dir.to_str().unwrap(), project.path()).unwrap();
                indexer.set_fallback_only(true);
                black_box(indexer.index_differential().unwrap())
            },
        );
    });

    group.finish();
}

/// ファイル数によるスケーリングのベンチマーク
fn benchmark_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    group.sample_size(5); // さらに少ないサンプル数

    for file_count in [10, 20, 30, 40, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(file_count),
            file_count,
            |b, &file_count| {
                b.iter_with_setup(
                    || {
                        let project = create_realistic_project();
                        // 追加ファイルを作成
                        for i in 0..file_count {
                            let path = project.path().join(format!("src/generated_{}.rs", i));
                            fs::write(
                                &path,
                                format!(
                                    r#"
// Generated file {}
pub fn function_{}() -> i32 {{
    {}
}}

pub struct Struct{} {{
    pub field: i32,
}}

impl Struct{} {{
    pub fn method(&self) -> i32 {{
        self.field
    }}
}}
"#,
                                    i, i, i, i, i
                                ),
                            )
                            .unwrap();
                        }
                        let db_dir = project.path().join("test_db");
                        (project, db_dir)
                    },
                    |(project, db_dir)| {
                        let mut indexer =
                            DifferentialIndexer::new(db_dir.to_str().unwrap(), project.path())
                                .unwrap();
                        indexer.set_fallback_only(true);
                        black_box(indexer.full_reindex().unwrap())
                    },
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_index_creation,
    benchmark_differential_index,
    benchmark_scaling
);
criterion_main!(benches);
