use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tracing::info;

/// LSP操作タイプ（パフォーマンス分析に基づく）
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum LspOperation {
    Initialize,
    WorkspaceSymbol,
    DocumentSymbol,
    Definition,
    References,
    TypeDefinition,
    Implementation,
    CallHierarchy,
}

impl LspOperation {
    /// パフォーマンス分析に基づくデフォルトタイムアウト設定
    pub fn default_timeouts(&self) -> (Duration, Duration, Duration) {
        match self {
            // (初回, 通常, 最大)
            Self::Initialize => (
                Duration::from_secs(5),
                Duration::from_secs(2),
                Duration::from_secs(30),
            ),
            Self::WorkspaceSymbol => (
                Duration::from_secs(2),
                Duration::from_millis(500),
                Duration::from_secs(5),
            ),
            Self::DocumentSymbol => (
                Duration::from_secs(1),
                Duration::from_millis(200),
                Duration::from_secs(2),
            ),
            Self::Definition | Self::References => (
                Duration::from_millis(1500),
                Duration::from_millis(300),
                Duration::from_secs(3),
            ),
            Self::TypeDefinition | Self::Implementation => (
                Duration::from_secs(2),
                Duration::from_millis(500),
                Duration::from_secs(4),
            ),
            Self::CallHierarchy => (
                Duration::from_secs(3),
                Duration::from_secs(1),
                Duration::from_secs(5),
            ),
        }
    }
}

/// LSP処理時間予測器
/// ファイルサイズと処理時間の履歴から適応的にタイムアウトを計算
#[derive(Debug, Clone)]
pub struct TimeoutPredictor {
    /// 操作タイプ別の処理履歴
    operation_history: HashMap<LspOperation, VecDeque<ProcessingRecord>>,
    /// 全体の処理履歴（後方互換性のため）
    history: VecDeque<ProcessingRecord>,
    /// 最大履歴数
    max_history: usize,
    /// 操作タイプ別の現在のタイムアウト設定
    operation_timeouts: HashMap<LspOperation, AdaptiveTimeout>,
    /// バイトあたりの予測処理時間（ミリ秒）
    ms_per_byte: f64,
    /// 行あたりの予測処理時間（ミリ秒）
    ms_per_line: f64,
    /// 言語別の速度係数
    language_speed: HashMap<String, f64>,
    /// 最大タイムアウト秒数
    max_timeout_secs: u64,
}

#[derive(Debug, Clone)]
struct ProcessingRecord {
    file_size: usize,
    line_count: usize,
    duration_ms: u64,
    success: bool,
}

#[derive(Debug, Clone)]
struct AdaptiveTimeout {
    current: Duration,
    initial: Duration,
    normal: Duration,
    max: Duration,
    success_count: u32,
    failure_count: u32,
}

impl Default for TimeoutPredictor {
    fn default() -> Self {
        let mut operation_timeouts = HashMap::new();

        // 各操作タイプのデフォルトタイムアウトを設定
        for op in [
            LspOperation::Initialize,
            LspOperation::WorkspaceSymbol,
            LspOperation::DocumentSymbol,
            LspOperation::Definition,
            LspOperation::References,
            LspOperation::TypeDefinition,
            LspOperation::Implementation,
            LspOperation::CallHierarchy,
        ] {
            let (initial, normal, max) = op.default_timeouts();
            operation_timeouts.insert(
                op,
                AdaptiveTimeout {
                    current: initial,
                    initial,
                    normal,
                    max,
                    success_count: 0,
                    failure_count: 0,
                },
            );
        }

        Self {
            operation_history: HashMap::new(),
            history: VecDeque::new(),
            max_history: 100,
            operation_timeouts,
            ms_per_byte: 0.001, // 初期値: 1KBあたり1ms
            ms_per_line: 0.5,   // 初期値: 1行あたり0.5ms
            language_speed: HashMap::new(),
            max_timeout_secs: 30,
        }
    }
}

impl TimeoutPredictor {
    pub fn new() -> Self {
        Self::default()
    }

    /// 操作タイプに応じた適応的タイムアウトを取得
    pub fn get_timeout(&self, operation: LspOperation) -> Duration {
        self.operation_timeouts
            .get(&operation)
            .map(|t| t.current)
            .unwrap_or_else(|| operation.default_timeouts().0)
    }

    /// 操作の結果を記録し、タイムアウトを適応的に調整
    pub fn record_operation(
        &mut self,
        operation: LspOperation,
        file_size: usize,
        line_count: usize,
        duration: Duration,
        success: bool,
    ) {
        // 履歴に追加
        let history = self
            .operation_history
            .entry(operation)
            .or_insert_with(|| VecDeque::with_capacity(self.max_history));

        if history.len() >= self.max_history {
            history.pop_front();
        }

        history.push_back(ProcessingRecord {
            file_size,
            line_count,
            duration_ms: duration.as_millis() as u64,
            success,
        });

        // タイムアウトを適応的に調整
        if let Some(timeout) = self.operation_timeouts.get_mut(&operation) {
            if success {
                timeout.success_count += 1;

                // 成功が続いたらタイムアウトを短縮
                if timeout.success_count > 10 {
                    timeout.current = timeout.normal;
                    info!(
                        "Adaptive timeout for {:?}: using normal timeout {:?}",
                        operation, timeout.normal
                    );
                }
            } else {
                timeout.failure_count += 1;

                // 失敗が多い場合はタイムアウトを延長
                if timeout.failure_count > 3 {
                    let new_timeout =
                        (timeout.current.as_secs_f64() * 1.5).min(timeout.max.as_secs_f64());
                    timeout.current = Duration::from_secs_f64(new_timeout);
                    info!(
                        "Adaptive timeout for {:?}: increased to {:?}",
                        operation, timeout.current
                    );
                }
            }
        }
    }

    /// ファイルサイズと行数から処理時間を予測
    pub fn predict_timeout(&self, file_size: usize, line_count: usize) -> Duration {
        // ファイルサイズと行数に基づく予測
        let size_ms = file_size as f64 * self.ms_per_byte;
        let line_ms = line_count as f64 * self.ms_per_line;

        // サイズと行数の両方を考慮（重み付き平均）
        let predicted_ms = (size_ms * 0.3 + line_ms * 0.7) as u64;

        // 安全マージン（1.5倍）を追加
        let timeout_ms = (predicted_ms as f64 * 1.5).max(200.0) as u64; // 最低200ms

        Duration::from_millis(timeout_ms)
    }

    /// 操作タイプとファイルサイズを考慮した予測
    pub fn predict_timeout_for_operation(
        &self,
        operation: LspOperation,
        file_size: usize,
        line_count: usize,
    ) -> Duration {
        // ベースタイムアウトを取得
        let base_timeout = self.get_timeout(operation);

        // ファイルサイズに基づく調整
        let size_factor = match operation {
            LspOperation::WorkspaceSymbol => 1.0, // プロジェクト全体なのでサイズに依存しない
            LspOperation::DocumentSymbol => (file_size as f64 / 10_000.0).max(1.0),
            _ => (file_size as f64 / 50_000.0).max(1.0),
        };

        let adjusted = base_timeout.as_secs_f64() * size_factor;
        Duration::from_secs_f64(adjusted)
    }

    /// 処理結果を記録して学習
    pub fn record_processing(
        &mut self,
        file_size: usize,
        line_count: usize,
        actual_duration: Duration,
    ) {
        let duration_ms = actual_duration.as_millis() as u64;

        // 履歴に追加
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }

        self.history.push_back(ProcessingRecord {
            file_size,
            line_count,
            duration_ms,
            success: true, // デフォルトでtrueとする
        });

        // 統計を更新
        self.update_statistics();
    }

    /// 履歴から統計情報を更新
    fn update_statistics(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let mut total_bytes_per_ms = 0.0;
        let mut total_lines_per_ms = 0.0;
        let mut count = 0;

        for record in &self.history {
            if record.duration_ms > 0 {
                total_bytes_per_ms += record.duration_ms as f64 / record.file_size.max(1) as f64;
                total_lines_per_ms += record.duration_ms as f64 / record.line_count.max(1) as f64;
                count += 1;
            }
        }

        if count > 0 {
            // 移動平均で更新（既存の値と新しい値の重み付き平均）
            let new_ms_per_byte = total_bytes_per_ms / count as f64;
            let new_ms_per_line = total_lines_per_ms / count as f64;

            self.ms_per_byte = self.ms_per_byte * 0.3 + new_ms_per_byte * 0.7;
            self.ms_per_line = self.ms_per_line * 0.3 + new_ms_per_line * 0.7;
        }
    }

    /// 現在の統計情報を取得
    pub fn get_statistics(&self) -> PredictorStatistics {
        PredictorStatistics {
            history_count: self.history.len(),
            avg_ms_per_byte: self.ms_per_byte,
            avg_ms_per_line: self.ms_per_line,
            total_files_processed: self.history.len(),
        }
    }

    /// バッチ処理用：複数ファイルの合計タイムアウトを予測
    pub fn predict_batch_timeout(&self, files: &[(usize, usize)]) -> Duration {
        let mut total_timeout = Duration::from_secs(0);

        for (file_size, line_count) in files {
            total_timeout += self.predict_timeout(*file_size, *line_count);
        }

        // バッチ処理の並列化を考慮（並列度4を仮定）
        let parallel_factor = 4.0;
        let adjusted_timeout = total_timeout.as_secs_f64() / parallel_factor;

        Duration::from_secs_f64(adjusted_timeout).min(Duration::from_secs(self.max_timeout_secs))
    }

    /// 処理の進捗を表示するための予測時間を取得
    pub fn format_eta(
        &self,
        processed: usize,
        total: usize,
        avg_file_size: usize,
        avg_lines: usize,
    ) -> String {
        if processed >= total {
            return "完了".to_string();
        }

        let remaining = total - processed;
        let timeout_per_file = self.predict_timeout(avg_file_size, avg_lines);
        let total_remaining = timeout_per_file.as_secs() * remaining as u64;

        if total_remaining < 60 {
            format!("残り約{}秒", total_remaining)
        } else if total_remaining < 3600 {
            format!("残り約{}分", total_remaining / 60)
        } else {
            format!(
                "残り約{}時間{}分",
                total_remaining / 3600,
                (total_remaining % 3600) / 60
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct PredictorStatistics {
    pub history_count: usize,
    pub avg_ms_per_byte: f64,
    pub avg_ms_per_line: f64,
    pub total_files_processed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_prediction() {
        let predictor = TimeoutPredictor::new();

        // 小さいファイル - 最小値は200ms
        let timeout = predictor.predict_timeout(1000, 50);
        assert!(timeout >= Duration::from_millis(200));
        assert!(timeout <= Duration::from_secs(10));

        // 大きいファイル
        let timeout = predictor.predict_timeout(100_000, 5000);
        assert!(timeout >= Duration::from_millis(200));
        assert!(timeout <= Duration::from_secs(60));
    }

    #[test]
    fn test_learning() {
        let mut predictor = TimeoutPredictor::new();

        // 処理履歴を記録
        predictor.record_processing(10_000, 500, Duration::from_millis(100));
        predictor.record_processing(20_000, 1000, Duration::from_millis(200));
        predictor.record_processing(5_000, 250, Duration::from_millis(50));

        // 統計が更新されていることを確認
        let stats = predictor.get_statistics();
        assert_eq!(stats.history_count, 3);
        assert!(stats.avg_ms_per_byte > 0.0);
        assert!(stats.avg_ms_per_line > 0.0);
    }

    #[test]
    fn test_batch_prediction() {
        let predictor = TimeoutPredictor::new();

        let files = vec![(10_000, 500), (20_000, 1000), (5_000, 250)];

        let batch_timeout = predictor.predict_batch_timeout(&files);
        assert!(batch_timeout > Duration::from_secs(0));
        assert!(batch_timeout <= Duration::from_secs(60));
    }
}
