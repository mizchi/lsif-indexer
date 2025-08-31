use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// LSP操作の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LspOperationType {
    Initialize,
    DocumentSymbol,
    Definition,
    References,
    TypeDefinition,
    Implementation,
    CallHierarchy,
    WorkspaceSymbol,
    Completion,
    Hover,
    SignatureHelp,
    Rename,
    Format,
    Other,
}

/// LSPプロセスのヘルスチェックと性能測定
pub struct LspHealthChecker {
    /// 初期化時のレスポンス時間
    init_response_time: Option<Duration>,
    /// 通常操作のレスポンス時間の履歴
    operation_times: Vec<Duration>,
    /// ウォームアップ中のレスポンス時間（初期化後の最初の数回）
    warmup_times: Vec<Duration>,
    /// 操作種別ごとのレスポンス時間
    operation_type_times: HashMap<LspOperationType, Vec<Duration>>,
    /// ウォームアップ期間のリクエスト数
    warmup_requests: usize,
    /// 現在のリクエスト数
    request_count: usize,
    /// 最大履歴数
    max_history: usize,
}

impl LspHealthChecker {
    pub fn new() -> Self {
        Self {
            init_response_time: None,
            operation_times: Vec::new(),
            warmup_times: Vec::new(),
            operation_type_times: HashMap::new(),
            warmup_requests: 5,  // 初期化後の最初の5リクエストはウォームアップ期間
            request_count: 0,
            max_history: 100,
        }
    }

    /// プロセスが正常に起動しているかチェック
    pub fn check_process_alive(child: &mut Child) -> Result<()> {
        match child.try_wait() {
            Ok(Some(status)) => {
                Err(anyhow!("LSP process exited with status: {:?}", status))
            }
            Ok(None) => {
                debug!("LSP process is running");
                Ok(())
            }
            Err(e) => {
                Err(anyhow!("Failed to check LSP process status: {}", e))
            }
        }
    }

    /// 初期ハンドシェイクを実行してプロセスの応答性を確認
    pub fn perform_handshake(
        stdin: &mut dyn Write,
        stdout: &mut BufReader<impl BufRead>,
    ) -> Result<Duration> {
        let start = Instant::now();
        
        // シンプルなテストメッセージを送信
        let test_message = r#"{"jsonrpc":"2.0","id":0,"method":"$/test"}"#;
        let content_length = test_message.len();
        
        writeln!(stdin, "Content-Length: {}\r", content_length)?;
        writeln!(stdin, "\r")?;
        stdin.write_all(test_message.as_bytes())?;
        stdin.flush()?;
        
        // レスポンスを待つ（タイムアウト付き）
        let timeout = Duration::from_secs(5);
        let mut waited = Duration::ZERO;
        
        loop {
            if waited >= timeout {
                warn!("Handshake timeout after {:?}", timeout);
                break;
            }
            
            // ヘッダーを読む
            let mut line = String::new();
            match stdout.read_line(&mut line) {
                Ok(0) => {
                    return Err(anyhow!("LSP process closed stdout"));
                }
                Ok(_) => {
                    if line.starts_with("Content-Length:") {
                        // レスポンスが来た
                        let response_time = start.elapsed();
                        info!("LSP handshake completed in {:?}", response_time);
                        return Ok(response_time);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    waited += Duration::from_millis(10);
                }
                Err(e) => {
                    return Err(anyhow!("Failed to read handshake response: {}", e));
                }
            }
        }
        
        // タイムアウトしたが、プロセスは生きている
        Ok(timeout)
    }

    /// 初期化時間を記録
    pub fn record_init_time(&mut self, duration: Duration) {
        self.init_response_time = Some(duration);
        info!("LSP initialization took {:?}", duration);
    }

    /// レスポンス時間を記録（操作種別を考慮）
    pub fn record_response_time(&mut self, duration: Duration) {
        self.record_response_time_for_operation(duration, LspOperationType::Other);
    }
    
    /// 操作種別を指定してレスポンス時間を記録
    pub fn record_response_time_for_operation(&mut self, duration: Duration, op_type: LspOperationType) {
        self.request_count += 1;
        
        // ウォームアップ期間中
        if self.request_count <= self.warmup_requests {
            self.warmup_times.push(duration);
            debug!("Warmup request #{} ({:?}): {:?}", self.request_count, op_type, duration);
        } else {
            // 通常操作
            self.operation_times.push(duration);
            if self.operation_times.len() > self.max_history {
                self.operation_times.remove(0);
            }
            
            // 操作種別ごとの記録
            let type_times = self.operation_type_times.entry(op_type).or_insert_with(Vec::new);
            type_times.push(duration);
            if type_times.len() > self.max_history / 10 {  // 種別ごとは少なめに保持
                type_times.remove(0);
            }
            
            debug!("Operation {:?}: {:?}", op_type, duration);
        }
    }

    /// 通常操作の平均レスポンス時間を計算
    pub fn average_response_time(&self) -> Option<Duration> {
        if self.operation_times.is_empty() {
            // 通常操作の履歴がない場合、ウォームアップ時間を使用
            if !self.warmup_times.is_empty() {
                let total: Duration = self.warmup_times.iter().sum();
                return Some(total / self.warmup_times.len() as u32);
            }
            return None;
        }
        
        let total: Duration = self.operation_times.iter().sum();
        Some(total / self.operation_times.len() as u32)
    }
    
    /// ウォームアップ期間の平均レスポンス時間
    pub fn average_warmup_time(&self) -> Option<Duration> {
        if self.warmup_times.is_empty() {
            return None;
        }
        let total: Duration = self.warmup_times.iter().sum();
        Some(total / self.warmup_times.len() as u32)
    }

    /// 適応的タイムアウトを計算（平均の3倍、最小3秒、最大60秒）
    pub fn calculate_adaptive_timeout(&self) -> Duration {
        match self.average_response_time() {
            Some(avg) => {
                let timeout = avg * 3;
                timeout.max(Duration::from_secs(3))
                       .min(Duration::from_secs(60))
            }
            None => Duration::from_secs(10), // デフォルト
        }
    }

    /// 初期化用のタイムアウトを計算
    pub fn calculate_init_timeout(&self) -> Duration {
        // 既に初期化時間が記録されている場合はそれを基準に
        if let Some(init_time) = self.init_response_time {
            let timeout = init_time * 3;  // 初期化時間の3倍
            return timeout.max(Duration::from_secs(10))
                         .min(Duration::from_secs(120));
        }
        
        // デフォルト: 言語別に調整可能
        Duration::from_secs(30)
    }
    
    /// ウォームアップ期間用のタイムアウトを計算
    pub fn calculate_warmup_timeout(&self) -> Duration {
        // ウォームアップ期間は初期化時間を基準に
        if let Some(init_time) = self.init_response_time {
            // 初期化時間の2倍（通常操作よりは長め）
            let timeout = init_time * 2;
            return timeout.max(Duration::from_secs(5))
                         .min(Duration::from_secs(30));
        }
        
        // ウォームアップ時間の平均がある場合
        if let Some(avg_warmup) = self.average_warmup_time() {
            let timeout = avg_warmup * 3;
            return timeout.max(Duration::from_secs(5))
                         .min(Duration::from_secs(30));
        }
        
        Duration::from_secs(10)  // デフォルト
    }
    
    /// 操作種別ごとの平均レスポンス時間
    pub fn average_response_time_for_operation(&self, op_type: LspOperationType) -> Option<Duration> {
        self.operation_type_times.get(&op_type).and_then(|times| {
            if times.is_empty() {
                None
            } else {
                let total: Duration = times.iter().sum();
                Some(total / times.len() as u32)
            }
        })
    }
    
    /// 操作種別に応じたタイムアウトを計算
    pub fn calculate_timeout_for_operation(&self, op_type: LspOperationType) -> Duration {
        // 操作種別ごとの履歴がある場合はそれを使用
        if let Some(avg) = self.average_response_time_for_operation(op_type) {
            let multiplier = match op_type {
                LspOperationType::Initialize => 5,
                LspOperationType::WorkspaceSymbol => 4,  // 全体検索は時間がかかる
                LspOperationType::References => 4,       // 参照検索も時間がかかる
                LspOperationType::CallHierarchy => 3,    
                LspOperationType::DocumentSymbol => 3,
                LspOperationType::TypeDefinition => 3,
                LspOperationType::Implementation => 3,
                LspOperationType::Definition => 2,
                LspOperationType::Hover => 2,
                LspOperationType::Completion => 2,
                LspOperationType::SignatureHelp => 2,
                LspOperationType::Rename => 3,
                LspOperationType::Format => 3,
                LspOperationType::Other => 3,
            };
            
            let timeout = avg * multiplier;
            
            // 操作種別ごとの最小・最大値
            let (min, max) = match op_type {
                LspOperationType::Initialize => (Duration::from_secs(10), Duration::from_secs(120)),
                LspOperationType::WorkspaceSymbol | LspOperationType::References => 
                    (Duration::from_secs(5), Duration::from_secs(60)),
                LspOperationType::CallHierarchy | LspOperationType::DocumentSymbol => 
                    (Duration::from_secs(3), Duration::from_secs(30)),
                _ => (Duration::from_secs(2), Duration::from_secs(20)),
            };
            
            return timeout.max(min).min(max);
        }
        
        // デフォルト値（操作種別ごと）
        match op_type {
            LspOperationType::Initialize => Duration::from_secs(30),
            LspOperationType::WorkspaceSymbol => Duration::from_secs(20),
            LspOperationType::References => Duration::from_secs(15),
            LspOperationType::CallHierarchy => Duration::from_secs(10),
            LspOperationType::DocumentSymbol => Duration::from_secs(10),
            LspOperationType::TypeDefinition => Duration::from_secs(8),
            LspOperationType::Implementation => Duration::from_secs(8),
            LspOperationType::Definition => Duration::from_secs(5),
            LspOperationType::Hover => Duration::from_secs(3),
            LspOperationType::Completion => Duration::from_secs(3),
            LspOperationType::SignatureHelp => Duration::from_secs(3),
            LspOperationType::Rename => Duration::from_secs(10),
            LspOperationType::Format => Duration::from_secs(10),
            LspOperationType::Other => Duration::from_secs(10),
        }
    }
    
    /// 現在のフェーズに応じた適切なタイムアウトを取得
    pub fn get_current_timeout(&self) -> Duration {
        if self.request_count == 0 {
            // 初期化前
            self.calculate_init_timeout()
        } else if self.request_count <= self.warmup_requests {
            // ウォームアップ期間
            self.calculate_warmup_timeout()
        } else {
            // 通常操作
            self.calculate_adaptive_timeout()
        }
    }
    
    /// 操作種別とフェーズを考慮したタイムアウトを取得
    pub fn get_timeout_for_operation(&self, op_type: LspOperationType) -> Duration {
        if op_type == LspOperationType::Initialize {
            return self.calculate_init_timeout();
        }
        
        if self.request_count <= self.warmup_requests {
            // ウォームアップ期間は少し長めに
            let base = self.calculate_timeout_for_operation(op_type);
            return base * 3 / 2;  // 1.5倍
        }
        
        self.calculate_timeout_for_operation(op_type)
    }

    /// ヘルスステータスを取得
    pub fn get_health_status(&self) -> HealthStatus {
        HealthStatus {
            init_time: self.init_response_time,
            average_response_time: self.average_response_time(),
            average_warmup_time: self.average_warmup_time(),
            operation_sample_count: self.operation_times.len(),
            warmup_sample_count: self.warmup_times.len(),
            request_count: self.request_count,
            recommended_timeout: self.calculate_adaptive_timeout(),
            recommended_init_timeout: self.calculate_init_timeout(),
            recommended_warmup_timeout: self.calculate_warmup_timeout(),
            current_phase: if self.request_count == 0 {
                "initialization"
            } else if self.request_count <= self.warmup_requests {
                "warmup"
            } else {
                "normal"
            },
        }
    }
}

#[derive(Debug)]
pub struct HealthStatus {
    pub init_time: Option<Duration>,
    pub average_response_time: Option<Duration>,
    pub average_warmup_time: Option<Duration>,
    pub operation_sample_count: usize,
    pub warmup_sample_count: usize,
    pub request_count: usize,
    pub recommended_timeout: Duration,
    pub recommended_init_timeout: Duration,
    pub recommended_warmup_timeout: Duration,
    pub current_phase: &'static str,
}

/// LSP起動の段階的確認
pub struct LspStartupValidator {
    max_startup_wait: Duration,
}

impl LspStartupValidator {
    pub fn new() -> Self {
        Self {
            max_startup_wait: Duration::from_secs(10),
        }
    }

    /// LSPサーバーの起動を段階的に確認
    pub fn validate_startup(
        &self,
        child: &mut Child,
        language: &str,
    ) -> Result<StartupStatus> {
        let start = Instant::now();
        
        // ステップ1: プロセスが生きているか確認
        debug!("Step 1: Checking if {} LSP process is alive", language);
        LspHealthChecker::check_process_alive(child)?;
        
        // ステップ2: stdoutが準備できているか確認（少し待つ）
        debug!("Step 2: Waiting for {} LSP to be ready", language);
        std::thread::sleep(Duration::from_millis(100));
        
        // ステップ3: プロセスがまだ生きているか再確認
        debug!("Step 3: Re-checking {} LSP process health", language);
        LspHealthChecker::check_process_alive(child)?;
        
        let startup_time = start.elapsed();
        info!("{} LSP startup validated in {:?}", language, startup_time);
        
        Ok(StartupStatus {
            startup_time,
            process_alive: true,
        })
    }

    /// 起動待機（特定の言語サーバー用の調整）
    pub fn wait_for_startup(&self, language: &str) -> Duration {
        // 言語別の起動待機時間
        let wait_time = match language {
            "rust" => Duration::from_millis(500),  // rust-analyzerは起動が遅い
            "typescript" | "javascript" => Duration::from_millis(300),
            "python" => Duration::from_millis(200),
            "go" => Duration::from_millis(200),
            _ => Duration::from_millis(100),
        };
        
        debug!("Waiting {:?} for {} LSP startup", wait_time, language);
        std::thread::sleep(wait_time);
        wait_time
    }
}

#[derive(Debug)]
pub struct StartupStatus {
    pub startup_time: Duration,
    pub process_alive: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker() {
        let mut checker = LspHealthChecker::new();
        
        // レスポンス時間を記録
        checker.record_response_time(Duration::from_millis(100));
        checker.record_response_time(Duration::from_millis(200));
        checker.record_response_time(Duration::from_millis(150));
        
        // 平均を確認
        let avg = checker.average_response_time().unwrap();
        assert!(avg >= Duration::from_millis(100));
        assert!(avg <= Duration::from_millis(200));
        
        // 適応的タイムアウトを確認
        let timeout = checker.calculate_adaptive_timeout();
        assert!(timeout >= Duration::from_secs(3)); // 最小値
    }

    #[test]
    fn test_startup_validator() {
        let validator = LspStartupValidator::new();
        
        // 言語別の待機時間を確認
        let rust_wait = validator.wait_for_startup("rust");
        assert_eq!(rust_wait, Duration::from_millis(500));
        
        let ts_wait = validator.wait_for_startup("typescript");
        assert_eq!(ts_wait, Duration::from_millis(300));
    }
}