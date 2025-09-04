use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::info;

/// LSPメトリクス収集システム
#[derive(Clone)]
pub struct LspMetricsCollector {
    metrics: Arc<RwLock<LspMetrics>>,
}

/// LSPメトリクスデータ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspMetrics {
    /// 総リクエスト数
    pub total_requests: u64,
    /// 操作別のメトリクス
    pub operation_metrics: HashMap<String, OperationMetrics>,
    /// 言語別のメトリクス
    pub language_metrics: HashMap<String, LanguageMetrics>,
    /// キャッシュメトリクス
    pub cache_metrics: CacheMetrics,
    /// プールメトリクス
    pub pool_metrics: PoolMetrics,
    /// 開始時刻（UNIXタイムスタンプミリ秒）
    pub start_time_ms: u64,
    /// 内部用のInstant（シリアライズされない）
    #[serde(skip)]
    pub start_instant: Option<Instant>,
}

/// 操作別メトリクス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetrics {
    pub count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub avg_duration: Duration,
}

/// 言語別メトリクス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageMetrics {
    pub files_processed: u64,
    pub symbols_extracted: u64,
    pub total_duration: Duration,
    pub cache_hit_rate: f64,
    pub error_rate: f64,
}

/// キャッシュメトリクス
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheMetrics {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub total_cache_size_bytes: u64,
}

/// プールメトリクス
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PoolMetrics {
    pub total_instances: u64,
    pub active_instances: u64,
    pub idle_instances: u64,
    pub instance_creation_count: u64,
    pub instance_reuse_count: u64,
    pub instance_eviction_count: u64,
}

impl Default for LspMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            operation_metrics: HashMap::new(),
            language_metrics: HashMap::new(),
            cache_metrics: CacheMetrics::default(),
            pool_metrics: PoolMetrics::default(),
            start_time_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            start_instant: Some(Instant::now()),
        }
    }
}

impl Default for LspMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LspMetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(LspMetrics::default())),
        }
    }

    /// 操作の開始を記録
    pub fn record_operation_start(&self, operation: &str) -> OperationTimer {
        let now = Instant::now();
        OperationTimer {
            collector: self.clone(),
            operation: operation.to_string(),
            start_time: now,
            start_instant: now,
        }
    }

    /// 操作の完了を記録
    pub fn record_operation_complete(&self, operation: &str, duration: Duration, success: bool) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.total_requests += 1;

        let op_metrics = metrics
            .operation_metrics
            .entry(operation.to_string())
            .or_insert_with(|| OperationMetrics {
                count: 0,
                success_count: 0,
                failure_count: 0,
                total_duration: Duration::ZERO,
                min_duration: Duration::from_secs(u64::MAX),
                max_duration: Duration::ZERO,
                avg_duration: Duration::ZERO,
            });

        op_metrics.count += 1;
        if success {
            op_metrics.success_count += 1;
        } else {
            op_metrics.failure_count += 1;
        }

        op_metrics.total_duration += duration;
        op_metrics.min_duration = op_metrics.min_duration.min(duration);
        op_metrics.max_duration = op_metrics.max_duration.max(duration);
        op_metrics.avg_duration = op_metrics.total_duration / op_metrics.count as u32;
    }

    /// 言語別メトリクスを記録
    pub fn record_language_metrics(
        &self,
        language: &str,
        files: u64,
        symbols: u64,
        duration: Duration,
    ) {
        let mut metrics = self.metrics.write().unwrap();

        let lang_metrics = metrics
            .language_metrics
            .entry(language.to_string())
            .or_insert_with(|| LanguageMetrics {
                files_processed: 0,
                symbols_extracted: 0,
                total_duration: Duration::ZERO,
                cache_hit_rate: 0.0,
                error_rate: 0.0,
            });

        lang_metrics.files_processed += files;
        lang_metrics.symbols_extracted += symbols;
        lang_metrics.total_duration += duration;
    }

    /// キャッシュヒットを記録
    pub fn record_cache_hit(&self, level: CacheLevel) {
        let mut metrics = self.metrics.write().unwrap();
        match level {
            CacheLevel::L1 => metrics.cache_metrics.l1_hits += 1,
            CacheLevel::L2 => metrics.cache_metrics.l2_hits += 1,
            CacheLevel::L3 => metrics.cache_metrics.l3_hits += 1,
        }
    }

    /// キャッシュミスを記録
    pub fn record_cache_miss(&self, level: CacheLevel) {
        let mut metrics = self.metrics.write().unwrap();
        match level {
            CacheLevel::L1 => metrics.cache_metrics.l1_misses += 1,
            CacheLevel::L2 => metrics.cache_metrics.l2_misses += 1,
            CacheLevel::L3 => metrics.cache_metrics.l3_misses += 1,
        }
    }

    /// プールメトリクスを更新
    pub fn update_pool_metrics(
        &self,
        total: u64,
        active: u64,
        idle: u64,
        created: u64,
        reused: u64,
        evicted: u64,
    ) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.pool_metrics = PoolMetrics {
            total_instances: total,
            active_instances: active,
            idle_instances: idle,
            instance_creation_count: created,
            instance_reuse_count: reused,
            instance_eviction_count: evicted,
        };
    }

    /// メトリクスのサマリーを取得
    pub fn get_summary(&self) -> MetricsSummary {
        let metrics = self.metrics.read().unwrap();
        let uptime = metrics
            .start_instant
            .map(|instant| instant.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        let total_cache_requests = metrics.cache_metrics.l1_hits
            + metrics.cache_metrics.l1_misses
            + metrics.cache_metrics.l2_hits
            + metrics.cache_metrics.l2_misses
            + metrics.cache_metrics.l3_hits
            + metrics.cache_metrics.l3_misses;

        let total_cache_hits = metrics.cache_metrics.l1_hits
            + metrics.cache_metrics.l2_hits
            + metrics.cache_metrics.l3_hits;

        let overall_cache_hit_rate = if total_cache_requests > 0 {
            total_cache_hits as f64 / total_cache_requests as f64
        } else {
            0.0
        };

        let mut total_success = 0;
        let mut total_failure = 0;
        let mut avg_response_time = Duration::ZERO;
        let mut op_count = 0;

        for op_metrics in metrics.operation_metrics.values() {
            total_success += op_metrics.success_count;
            total_failure += op_metrics.failure_count;
            avg_response_time += op_metrics.avg_duration;
            op_count += 1;
        }

        if op_count > 0 {
            avg_response_time /= op_count as u32;
        }

        let error_rate = if metrics.total_requests > 0 {
            total_failure as f64 / metrics.total_requests as f64
        } else {
            0.0
        };

        MetricsSummary {
            uptime,
            total_requests: metrics.total_requests,
            average_response_time: avg_response_time,
            cache_hit_rate: overall_cache_hit_rate,
            error_rate,
            pool_efficiency: if metrics.pool_metrics.instance_creation_count > 0 {
                metrics.pool_metrics.instance_reuse_count as f64
                    / (metrics.pool_metrics.instance_creation_count
                        + metrics.pool_metrics.instance_reuse_count) as f64
            } else {
                0.0
            },
        }
    }

    /// メトリクスをレポート形式で出力
    pub fn print_report(&self) {
        let summary = self.get_summary();
        let metrics = self.metrics.read().unwrap();

        println!("\n=== LSP Performance Metrics Report ===");
        println!("Uptime: {:.2}s", summary.uptime.as_secs_f64());
        println!("Total Requests: {}", summary.total_requests);
        println!(
            "Average Response Time: {:.3}ms",
            summary.average_response_time.as_millis()
        );
        println!(
            "Overall Cache Hit Rate: {:.1}%",
            summary.cache_hit_rate * 100.0
        );
        println!("Error Rate: {:.1}%", summary.error_rate * 100.0);
        println!(
            "Pool Reuse Efficiency: {:.1}%",
            summary.pool_efficiency * 100.0
        );

        println!("\n## Operation Breakdown");
        for (op_name, op_metrics) in &metrics.operation_metrics {
            println!(
                "  {}: {} requests, avg {:.3}ms, success rate {:.1}%",
                op_name,
                op_metrics.count,
                op_metrics.avg_duration.as_millis(),
                (op_metrics.success_count as f64 / op_metrics.count as f64) * 100.0
            );
        }

        println!("\n## Language Performance");
        for (lang, lang_metrics) in &metrics.language_metrics {
            let avg_time_per_file = if lang_metrics.files_processed > 0 {
                lang_metrics.total_duration / lang_metrics.files_processed as u32
            } else {
                Duration::ZERO
            };

            println!(
                "  {}: {} files, {} symbols, avg {:.3}ms/file",
                lang,
                lang_metrics.files_processed,
                lang_metrics.symbols_extracted,
                avg_time_per_file.as_millis()
            );
        }

        println!("\n## Cache Statistics");
        println!(
            "  L1: {} hits, {} misses ({:.1}% hit rate)",
            metrics.cache_metrics.l1_hits,
            metrics.cache_metrics.l1_misses,
            cache_hit_rate(
                metrics.cache_metrics.l1_hits,
                metrics.cache_metrics.l1_misses
            ) * 100.0
        );
        println!(
            "  L2: {} hits, {} misses ({:.1}% hit rate)",
            metrics.cache_metrics.l2_hits,
            metrics.cache_metrics.l2_misses,
            cache_hit_rate(
                metrics.cache_metrics.l2_hits,
                metrics.cache_metrics.l2_misses
            ) * 100.0
        );
        println!(
            "  L3: {} hits, {} misses ({:.1}% hit rate)",
            metrics.cache_metrics.l3_hits,
            metrics.cache_metrics.l3_misses,
            cache_hit_rate(
                metrics.cache_metrics.l3_hits,
                metrics.cache_metrics.l3_misses
            ) * 100.0
        );

        println!("\n## Pool Statistics");
        println!(
            "  Total Instances: {}",
            metrics.pool_metrics.total_instances
        );
        println!(
            "  Active: {}, Idle: {}",
            metrics.pool_metrics.active_instances, metrics.pool_metrics.idle_instances
        );
        println!(
            "  Created: {}, Reused: {}, Evicted: {}",
            metrics.pool_metrics.instance_creation_count,
            metrics.pool_metrics.instance_reuse_count,
            metrics.pool_metrics.instance_eviction_count
        );
    }

    /// メトリクスをJSON形式でエクスポート
    pub fn export_json(&self) -> String {
        let metrics = self.metrics.read().unwrap();
        serde_json::to_string_pretty(&*metrics).unwrap_or_else(|_| "{}".to_string())
    }

    /// メトリクスをリセット
    pub fn reset(&self) {
        let mut metrics = self.metrics.write().unwrap();
        *metrics = LspMetrics::default();
        info!("LSP metrics reset");
    }
}

/// メトリクスサマリー
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub uptime: Duration,
    pub total_requests: u64,
    pub average_response_time: Duration,
    pub cache_hit_rate: f64,
    pub error_rate: f64,
    pub pool_efficiency: f64,
}

/// キャッシュレベル
#[derive(Debug, Clone, Copy)]
pub enum CacheLevel {
    L1,
    L2,
    L3,
}

/// 操作タイマー（自動記録）
pub struct OperationTimer {
    collector: LspMetricsCollector,
    operation: String,
    start_time: Instant,
    start_instant: Instant, // 追加
}

impl Drop for OperationTimer {
    fn drop(&mut self) {
        let duration = self.start_instant.elapsed();
        self.collector
            .record_operation_complete(&self.operation, duration, true);
    }
}

fn cache_hit_rate(hits: u64, misses: u64) -> f64 {
    let total = hits + misses;
    if total == 0 {
        0.0
    } else {
        hits as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collection() {
        let collector = LspMetricsCollector::new();

        // 操作を記録
        collector.record_operation_complete("initialize", Duration::from_millis(100), true);
        collector.record_operation_complete("workspace/symbol", Duration::from_millis(50), true);
        collector.record_operation_complete("workspace/symbol", Duration::from_millis(60), false);

        // キャッシュヒット/ミスを記録
        collector.record_cache_hit(CacheLevel::L1);
        collector.record_cache_miss(CacheLevel::L1);
        collector.record_cache_hit(CacheLevel::L2);

        let summary = collector.get_summary();
        assert_eq!(summary.total_requests, 3);
        assert!(summary.error_rate > 0.0);
        assert!(summary.cache_hit_rate > 0.0);
    }

    #[test]
    fn test_operation_timer() {
        let collector = LspMetricsCollector::new();

        {
            let _timer = collector.record_operation_start("test_operation");
            std::thread::sleep(Duration::from_millis(10));
        } // タイマーがドロップされて自動的に記録される

        let metrics = collector.metrics.read().unwrap();
        assert_eq!(
            metrics
                .operation_metrics
                .get("test_operation")
                .unwrap()
                .count,
            1
        );
    }
}
