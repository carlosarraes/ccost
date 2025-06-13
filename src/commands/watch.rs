use crate::cli::Cli;
use crate::config::Config;

pub async fn handle_watch_command(
    project_filter: Option<String>,
    expensive_threshold: f64,
    _no_charts: bool,
    refresh_rate_ms: u64,
    cli: &Cli,
) -> anyhow::Result<()> {
    use crate::watch::WatchMode;

    // Load configuration with CLI overrides
    let mut config = Config::load()?;

    // Apply CLI overrides
    if let Some(ref currency) = cli.currency {
        config.currency.default_currency = currency.clone();
    }
    if let Some(ref timezone) = cli.timezone {
        config.timezone.timezone = timezone.clone();
    }

    // Create and start watch mode
    let mut watch_mode =
        WatchMode::new(config, project_filter, expensive_threshold, refresh_rate_ms)?;

    // Start watching
    watch_mode.run().await?;

    Ok(())
}
