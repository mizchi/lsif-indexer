use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    // 環境変数でログレベルを設定可能にする
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn");
    }

    // ログ初期化
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .init();

    // CLIパース＆実行
    let cli = Cli::parse();
    cli.run()?;

    Ok(())
}
