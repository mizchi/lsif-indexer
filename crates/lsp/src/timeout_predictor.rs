use std::time::Duration;
use std::collections::VecDeque;

/// LSP処理時間予測器
/// ファイルサイズと処理時間の履歴から適応的にタイムアウトを計算
#[derive(Debug, Clone)]
pub struct TimeoutPredictor {
    /// 過去の処理履歴（ファイルサイズ、行数、処理時間）
    history: VecDeque<ProcessingRecord>,
    /// 最大履歴数
    max_history: usize,
    /// 基本タイムアウト（秒）
    base_timeout_secs: u64,
    /// 最小タイムアウト（秒）
    min_timeout_secs: u64,
    /// 最大タイムアウト（秒）
    max_timeout_secs: u64,
    /// バイトあたりの予測処理時間（ミリ秒）
    ms_per_byte: f64,
    /// 行あたりの予測処理時間（ミリ秒）
    ms_per_line: f64,
}

#[derive(Debug, Clone)]
struct ProcessingRecord {
    file_size: usize,
    line_count: usize,
    duration_ms: u64,
}

impl Default for TimeoutPredictor {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(100),
            max_history: 100,
            base_timeout_secs: 5,
            min_timeout_secs: 3,
            max_timeout_secs: 60,
            ms_per_byte: 0.001,  // 初期値: 1KBあたり1ms
            ms_per_line: 0.5,    // 初期値: 1行あたり0.5ms
        }
    }
}

impl TimeoutPredictor {
    pub fn new() -> Self {
        Self::default()
    }

    /// カスタム設定で初期化
    pub fn with_config(
        base_timeout_secs: u64,
        min_timeout_secs: u64,
        max_timeout_secs: u64,
    ) -> Self {
        Self {
            base_timeout_secs,
            min_timeout_secs,
            max_timeout_secs,
            ..Self::default()
        }
    }

    /// ファイルサイズと行数から処理時間を予測
    pub fn predict_timeout(&self, file_size: usize, line_count: usize) -> Duration {
        // 履歴がある場合は統計情報を使用
        let predicted_ms = if !self.history.is_empty() {
            let size_ms = file_size as f64 * self.ms_per_byte;
            let line_ms = line_count as f64 * self.ms_per_line;
            
            // サイズと行数の両方を考慮（重み付き平均）
            (size_ms * 0.3 + line_ms * 0.7) as u64
        } else {
            // 履歴がない場合は初期推定値を使用
            let base_ms = self.base_timeout_secs * 1000;
            let size_factor = (file_size as f64 / 10_000.0).max(1.0); // 10KBごとに係数増加
            let line_factor = (line_count as f64 / 500.0).max(1.0);   // 500行ごとに係数増加
            
            (base_ms as f64 * size_factor.max(line_factor)) as u64
        };

        // 安全マージン（1.5倍）を追加
        let timeout_ms = (predicted_ms as f64 * 1.5) as u64;
        
        // 最小値と最大値でクランプ
        let timeout_secs = (timeout_ms / 1000).max(self.min_timeout_secs).min(self.max_timeout_secs);
        
        Duration::from_secs(timeout_secs)
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
    pub fn format_eta(&self, processed: usize, total: usize, avg_file_size: usize, avg_lines: usize) -> String {
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
            format!("残り約{}時間{}分", total_remaining / 3600, (total_remaining % 3600) / 60)
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
        
        // 小さいファイル
        let timeout = predictor.predict_timeout(1000, 50);
        assert!(timeout >= Duration::from_secs(3));
        assert!(timeout <= Duration::from_secs(10));
        
        // 大きいファイル
        let timeout = predictor.predict_timeout(100_000, 5000);
        assert!(timeout >= Duration::from_secs(5));
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
        
        let files = vec![
            (10_000, 500),
            (20_000, 1000),
            (5_000, 250),
        ];
        
        let batch_timeout = predictor.predict_batch_timeout(&files);
        assert!(batch_timeout > Duration::from_secs(0));
        assert!(batch_timeout <= Duration::from_secs(60));
    }
}