#![allow(dead_code)]

mod ai;
mod alert;
mod app;
mod article;
mod auto_fetch;
mod config;
mod db;
mod enrichment;
mod feed;
mod keyword;
mod notify;
mod report;
mod scheduler;
mod tag;
mod template;
mod theme;
mod types;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "ThreatDeck",
    version,
    about = "Terminal-based threat intelligence monitoring and alerting platform"
)]
struct Cli {
    /// Print config paths and exit
    #[arg(long)]
    config_paths: bool,
    /// Run in headless daemon mode
    #[arg(long)]
    daemon: bool,
    /// Process queued enrichment jobs once and exit
    #[arg(long)]
    enrich_once: bool,
    /// Maximum enrichment jobs to process with --enrich-once
    #[arg(long, default_value_t = 25)]
    enrich_limit: i64,
    /// Print enrichment queue jobs and exit
    #[arg(long)]
    enrichment_queue: bool,
    /// Maximum enrichment queue jobs to print with --enrichment-queue
    #[arg(long, default_value_t = 50)]
    enrichment_queue_limit: i64,
    /// Print enrichment providers and exit
    #[arg(long)]
    enrichment_providers: bool,
    /// Enable an enrichment provider by name
    #[arg(long)]
    enable_provider: Option<String>,
    /// Disable an enrichment provider by name
    #[arg(long)]
    disable_provider: Option<String>,
    /// Validate and install a local CISA KEV JSON cache file
    #[arg(long)]
    import_cisa_kev: Option<PathBuf>,
    /// Print stored indicators and exit
    #[arg(long)]
    ioc_list: bool,
    /// Filter --ioc-list by indicator value text
    #[arg(long)]
    ioc_search: Option<String>,
    /// Show one stored indicator by value or normalized value
    #[arg(long)]
    ioc_show: Option<String>,
    /// Maximum indicators to print with --ioc-list
    #[arg(long, default_value_t = 50)]
    ioc_limit: i64,
    /// Export stored indicators as json or csv
    #[arg(long, value_parser = ["json", "csv"])]
    ioc_export: Option<String>,
    /// Extract IOCs from text and exit without storing them
    #[arg(long)]
    ioc_extract_text: Option<String>,
    /// Extract IOCs from a file and exit without storing them
    #[arg(long)]
    ioc_extract_file: Option<PathBuf>,
    /// Acknowledge an alert by ID
    #[arg(long)]
    alert_acknowledge: Option<i64>,
    /// Start investigating an alert by ID
    #[arg(long)]
    alert_investigate: Option<i64>,
    /// Escalate an alert by ID
    #[arg(long)]
    alert_escalate: Option<i64>,
    /// Close an alert by ID (requires --disposition)
    #[arg(long)]
    alert_close: Option<i64>,
    /// Set disposition when closing an alert (ConfirmedThreat, FalsePositive, Benign, Duplicate, Informational, NeedsMoreContext)
    #[arg(long)]
    disposition: Option<String>,
    /// Reason or note when closing an alert
    #[arg(long)]
    close_reason: Option<String>,
    /// Reopen a closed alert by ID
    #[arg(long)]
    alert_reopen: Option<i64>,
    /// Add a note to an alert by ID
    #[arg(long)]
    alert_note: Option<i64>,
    /// Note text for --alert-note
    #[arg(long)]
    note_text: Option<String>,
    /// Show triage history for an alert by ID
    #[arg(long)]
    alert_history: Option<i64>,
    /// List alerts with optional filters
    #[arg(long)]
    alert_list: bool,
    /// Filter --alert-list by status
    #[arg(long)]
    alert_status: Option<String>,
    /// Filter --alert-list by disposition
    #[arg(long)]
    alert_disposition: Option<String>,
    /// Filter --alert-list by owner
    #[arg(long)]
    alert_owner: Option<String>,
    /// Seed the database with demo data (feeds, keywords, alerts, tags)
    #[arg(long)]
    seed_demo: bool,
    /// Fetch one stored feed by ID, record diagnostics, print a report, and exit
    #[arg(long)]
    debug_feed_id: Option<i64>,
    /// Fetch one URL without storing it, print diagnostics, and exit
    #[arg(long)]
    check_feed: Option<String>,
    /// Export alert report as Markdown
    #[arg(long)]
    report_alert: Option<i64>,
    /// Export visible alerts as Markdown
    #[arg(long)]
    report_alerts_visible: bool,
    /// Export feed health report as Markdown
    #[arg(long)]
    report_feed_health: bool,
    /// Output path for report export
    #[arg(long)]
    report_output: Option<PathBuf>,
    /// Overwrite existing report file
    #[arg(long)]
    report_overwrite: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = config::Paths::new()?;

    if cli.config_paths {
        println!("config dir : {}", paths.config_dir.display());
        println!("data dir   : {}", paths.data_dir.display());
        println!("config     : {}", paths.config_file.display());
        println!("database   : {}", paths.db_file.display());
        return Ok(());
    }

    if let Some(text) = cli.ioc_extract_text.as_deref() {
        print_extracted_iocs("text", text);
        return Ok(());
    }

    if let Some(path) = cli.ioc_extract_file.as_deref() {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading IOC extraction file: {}", path.display()))?;
        print_extracted_iocs(path.to_string_lossy().as_ref(), &text);
        return Ok(());
    }

    paths.ensure_dirs().context("creating config/data dirs")?;

    let app_config = config::load_app_config(&paths.config_file)?;
    let db = db::Db::open(&paths.db_file)?;
    db.init_schema().context("initializing database schema")?;

    if let Some(id) = cli.debug_feed_id {
        debug_stored_feed(&db, id)?;
        return Ok(());
    }

    if let Some(url) = cli.check_feed.as_deref() {
        check_feed_url(url);
        return Ok(());
    }

    if cli.enrich_once {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("creating enrichment runtime")?;
        let processed = runtime.block_on(enrichment::run_enrichment_once(
            &db,
            &paths.data_dir,
            cli.enrich_limit,
        ))?;
        println!("Processed {processed} enrichment job(s).");
        return Ok(());
    }

    if cli.enrichment_queue {
        let jobs = db.list_enrichment_jobs(cli.enrichment_queue_limit)?;
        print_enrichment_queue(&jobs);
        return Ok(());
    }

    if cli.enrichment_providers {
        let providers = db.list_enrichment_providers()?;
        print_enrichment_providers(&providers);
        return Ok(());
    }

    if let Some(provider) = cli.enable_provider.as_deref() {
        db.set_enrichment_provider_enabled(provider, true)?;
        println!("Enabled enrichment provider: {provider}");
        return Ok(());
    }

    if let Some(provider) = cli.disable_provider.as_deref() {
        db.set_enrichment_provider_enabled(provider, false)?;
        println!("Disabled enrichment provider: {provider}");
        return Ok(());
    }

    if let Some(path) = cli.import_cisa_kev.as_deref() {
        import_cisa_kev_cache(path, &paths.data_dir)?;
        return Ok(());
    }

    if cli.ioc_list {
        let indicators = db.search_indicators(&db::IndicatorSearch {
            text: cli.ioc_search.clone(),
            indicator_type: None,
            limit: Some(cli.ioc_limit),
        })?;
        print_ioc_list(&db, &indicators);
        return Ok(());
    }

    if let Some(value) = cli.ioc_show.as_deref() {
        print_ioc_show(&db, value)?;
        return Ok(());
    }

    if let Some(format) = cli.ioc_export.as_deref() {
        let indicators = db.search_indicators(&db::IndicatorSearch {
            text: cli.ioc_search.clone(),
            indicator_type: None,
            limit: Some(cli.ioc_limit),
        })?;
        print_ioc_export(&db, &indicators, format)?;
        return Ok(());
    }

    if let Some(id) = cli.alert_acknowledge {
        db.update_alert_status(id, crate::types::AlertStatus::Acknowledged, None)?;
        println!("Alert {id} acknowledged.");
        return Ok(());
    }
    if let Some(id) = cli.alert_investigate {
        db.update_alert_status(id, crate::types::AlertStatus::Investigating, None)?;
        println!("Alert {id} marked as investigating.");
        return Ok(());
    }
    if let Some(id) = cli.alert_escalate {
        db.update_alert_status(id, crate::types::AlertStatus::Escalated, None)?;
        println!("Alert {id} escalated.");
        return Ok(());
    }
    if let Some(id) = cli.alert_close {
        let disp_str = cli.disposition.as_deref().unwrap_or("Unknown");
        let disposition = match disp_str {
            "ConfirmedThreat" => crate::types::AlertDisposition::ConfirmedThreat,
            "FalsePositive" => crate::types::AlertDisposition::FalsePositive,
            "Benign" => crate::types::AlertDisposition::Benign,
            "Duplicate" => crate::types::AlertDisposition::Duplicate,
            "Informational" => crate::types::AlertDisposition::Informational,
            "NeedsMoreContext" => crate::types::AlertDisposition::NeedsMoreContext,
            _ => crate::types::AlertDisposition::Unknown,
        };
        if disposition == crate::types::AlertDisposition::Unknown {
            anyhow::bail!("Closing an alert requires a non-Unknown disposition. Use --disposition");
        }
        db.close_alert(id, disposition, cli.close_reason.as_deref())?;
        println!("Alert {id} closed as {disp_str}.");
        return Ok(());
    }
    if let Some(id) = cli.alert_reopen {
        db.reopen_alert(id, None)?;
        println!("Alert {id} reopened.");
        return Ok(());
    }
    if let Some(id) = cli.alert_note {
        let note = cli.note_text.as_deref().unwrap_or("");
        if note.is_empty() {
            anyhow::bail!("--alert-note requires --note-text");
        }
        db.add_alert_note(id, note)?;
        println!("Note added to alert {id}.");
        return Ok(());
    }
    if let Some(id) = cli.alert_history {
        let events = db.list_alert_triage_events(id)?;
        if events.is_empty() {
            println!("No triage history for alert {id}.");
        } else {
            println!("Triage history for alert {id}:");
            for event in events {
                let ts = event.created_at.format("%Y-%m-%d %H:%M");
                let change = match (event.old_value.as_ref(), event.new_value.as_ref()) {
                    (Some(old), Some(new)) => format!("{old} -> {new}"),
                    (None, Some(new)) => format!("-> {new}"),
                    (Some(old), None) => format!("{old} -> (none)"),
                    (None, None) => String::new(),
                };
                let note = event.note.map(|n| format!(" | {n}")).unwrap_or_default();
                println!("  {ts}  {:20} {}{}", event.event_type, change, note);
            }
        }
        return Ok(());
    }
    if let Some(alert_id) = cli.report_alert {
        let report_service = crate::report::ReportService::new();
        let options = threatdeck_report::ReportExportOptions {
            report_type: threatdeck_report::ReportType::Alert,
            format: threatdeck_report::ExportFormat::Markdown,
            output_path: cli.report_output.clone(),
            include_raw_content: false,
            include_metadata: true,
            include_iocs: true,
            include_enrichment: true,
            include_triage_history: true,
            include_feed_health: false,
            include_tags: true,
            redact_secrets: true,
            overwrite: cli.report_overwrite,
            generated_by: None,
        };
        let export_dir = paths.data_dir.join("exports");
        match report_service.export_alert_report(&db, alert_id, &options, &export_dir) {
            Ok(result) => {
                println!("Exported alert report to: {}", result.path.display());
                println!("  Bytes written: {}", result.bytes_written);
            }
            Err(e) => {
                eprintln!("Export failed: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if cli.report_alerts_visible {
        let report_service = crate::report::ReportService::new();
        let filter = db::AlertFilter {
            limit: Some(500),
            ..db::AlertFilter::default()
        };
        let alerts = db.list_alerts(&filter)?;
        let options = threatdeck_report::ReportExportOptions {
            report_type: threatdeck_report::ReportType::AlertCollection,
            format: threatdeck_report::ExportFormat::Markdown,
            output_path: cli.report_output.clone(),
            include_raw_content: false,
            include_metadata: true,
            include_iocs: true,
            include_enrichment: true,
            include_triage_history: true,
            include_feed_health: false,
            include_tags: true,
            redact_secrets: true,
            overwrite: cli.report_overwrite,
            generated_by: None,
        };
        let export_dir = paths.data_dir.join("exports");
        match report_service.export_visible_alerts_report(&db, &alerts, &filter, &options, &export_dir) {
            Ok(result) => {
                println!("Exported {} alerts to: {}", alerts.len(), result.path.display());
                println!("  Bytes written: {}", result.bytes_written);
            }
            Err(e) => {
                eprintln!("Export failed: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if cli.report_feed_health {
        let report_service = crate::report::ReportService::new();
        let options = threatdeck_report::ReportExportOptions {
            report_type: threatdeck_report::ReportType::FeedHealth,
            format: threatdeck_report::ExportFormat::Markdown,
            output_path: cli.report_output.clone(),
            include_raw_content: false,
            include_metadata: true,
            include_iocs: false,
            include_enrichment: false,
            include_triage_history: false,
            include_feed_health: true,
            include_tags: false,
            redact_secrets: true,
            overwrite: cli.report_overwrite,
            generated_by: None,
        };
        let export_dir = paths.data_dir.join("exports");
        match report_service.export_feed_health_report(&db, &options, &export_dir) {
            Ok(result) => {
                println!("Exported feed health report to: {}", result.path.display());
                println!("  Bytes written: {}", result.bytes_written);
            }
            Err(e) => {
                eprintln!("Export failed: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    if cli.seed_demo {
        seed_demo_data(&db)?;
        println!("Demo data seeded.");
        println!("  Feeds:    {}", db.list_feeds(None)?.len());
        println!("  Keywords: {}", db.list_keywords(false)?.len());
        println!("  Alerts:   {}", db.get_alert_count()?);
        println!("  Tags:     {}", db.list_tags()?.len());
        return Ok(());
    }

    if cli.alert_list {
        let status = cli.alert_status.as_deref().map(|s| match s {
            "New" => crate::types::AlertStatus::New,
            "Acknowledged" => crate::types::AlertStatus::Acknowledged,
            "Investigating" => crate::types::AlertStatus::Investigating,
            "Escalated" => crate::types::AlertStatus::Escalated,
            "Closed" => crate::types::AlertStatus::Closed,
            _ => crate::types::AlertStatus::New,
        });
        let disposition = cli.alert_disposition.as_deref().map(|s| match s {
            "ConfirmedThreat" => crate::types::AlertDisposition::ConfirmedThreat,
            "FalsePositive" => crate::types::AlertDisposition::FalsePositive,
            "Benign" => crate::types::AlertDisposition::Benign,
            "Duplicate" => crate::types::AlertDisposition::Duplicate,
            "Informational" => crate::types::AlertDisposition::Informational,
            "NeedsMoreContext" => crate::types::AlertDisposition::NeedsMoreContext,
            _ => crate::types::AlertDisposition::Unknown,
        });
        let filter = db::AlertFilter {
            status,
            disposition,
            owner: cli.alert_owner.clone(),
            limit: Some(100),
            ..db::AlertFilter::default()
        };
        let alerts = db.list_alerts(&filter)?;
        if alerts.is_empty() {
            println!("No alerts found.");
        } else {
            println!(
                "{:>6} {:>8} {:>12} {:>14} {:>12} {:>20} {}",
                "ID", "Severity", "Status", "Disposition", "Owner", "Detected", "Title"
            );
            for a in alerts {
                let sev = format!("{:?}", a.alert.effective_severity());
                let status = format!("{:?}", a.alert.status);
                let disp = format!("{:?}", a.alert.disposition);
                let owner = a.alert.owner.as_deref().unwrap_or("-");
                let dt = a.alert.detected_at.format("%Y-%m-%d %H:%M");
                let title = a.alert.title.as_deref().unwrap_or("(untitled)");
                println!(
                    "{:>6} {:>8} {:>12} {:>14} {:>12} {:>20} {}",
                    a.alert.id, sev, status, disp, owner, dt, title
                );
            }
        }
        return Ok(());
    }

    if cli.daemon {
        println!("Daemon mode not yet implemented.");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new(db, app_config, paths);
    let res = run_app(&mut terminal, &mut app);
    app.stop_auto_fetch();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    res
}

fn debug_stored_feed(db: &db::Db, feed_id: i64) -> Result<()> {
    let feed = db
        .get_feed(feed_id)?
        .with_context(|| format!("feed not found: {feed_id}"))?;
    let template = match feed.api_template_id {
        Some(template_id) => db.get_template(template_id)?,
        None => None,
    };
    let outcome = feed::FeedManager::run_fetch_attempt(&feed, template);
    let content_hash = outcome
        .result
        .as_ref()
        .map(|result| result.content_hash.as_str());
    db.record_feed_fetch_outcome(feed.id, &outcome.attempt, content_hash)?;
    print!(
        "{}",
        format_feed_diagnostic_report(Some(&feed.name), &outcome.attempt)
    );
    Ok(())
}

fn check_feed_url(url: &str) {
    let feed = types::Feed {
        id: 0,
        name: "Ad hoc feed check".into(),
        url: url.to_string(),
        feed_type: types::FeedType::Rss,
        enabled: true,
        interval_secs: 300,
        last_fetch_at: None,
        last_error: None,
        last_fetch_success_at: None,
        last_fetch_failed_at: None,
        last_failure_phase: None,
        last_failure_kind: None,
        last_http_status: None,
        consecutive_failures: 0,
        content_hash: None,
        created_at: chrono::Utc::now(),
        api_template_id: None,
        api_key: None,
        custom_headers: None,
        tor_proxy: None,
    };
    let outcome = feed::FeedManager::run_fetch_attempt(&feed, None);
    print!("{}", format_feed_diagnostic_report(None, &outcome.attempt));
}

fn format_feed_diagnostic_report(
    feed_name: Option<&str>,
    attempt: &feed::diagnostics::FetchAttempt,
) -> String {
    let status = if attempt.success { "ok" } else { "failed" };
    let mut output = String::new();
    output.push_str("Feed diagnostic report\n");
    if let Some(feed_name) = feed_name {
        output.push_str(&format!("Feed: {feed_name}\n"));
    }
    output.push_str(&format!("URL: {}\n", attempt.url));
    if let Some(final_url) = attempt.final_url.as_deref() {
        output.push_str(&format!("Final URL: {final_url}\n"));
    }
    output.push_str(&format!("Result: {status}\n"));
    output.push_str(&format!("Elapsed: {}ms\n", attempt.elapsed_ms));
    if let Some(http_status) = attempt.http_status {
        output.push_str(&format!("HTTP status: {http_status}\n"));
    }

    if let Some(items_seen) = attempt.items_seen {
        output.push_str(&format!("Items seen: {items_seen}\n"));
    }
    if let Some(items_new) = attempt.items_new {
        output.push_str(&format!("Items new: {items_new}\n"));
    }

    if let Some(diagnostic) = &attempt.diagnostic {
        output.push_str(&format!("Phase: {}\n", diagnostic.phase.label()));
        output.push_str(&format!("Kind: {}\n", diagnostic.kind.label()));
        output.push_str(&format!("Summary: {}\n", diagnostic.summary));
        if let Some(detail) = diagnostic.detail.as_deref() {
            output.push_str(&format!("Detail: {detail}\n"));
        }
    }

    output
}

fn print_enrichment_queue(jobs: &[db::EnrichmentJobWithContext]) {
    if jobs.is_empty() {
        println!("No enrichment jobs.");
        return;
    }

    println!(
        "{:<6} {:<10} {:<14} {:<10} {:<8} {:<20} {:<32} Error",
        "ID", "Status", "Provider", "Type", "Attempts", "Next Attempt", "Indicator"
    );
    for job in jobs {
        println!(
            "{:<6} {:<10} {:<14} {:<10} {:<8} {:<20} {:<32} {}",
            job.id,
            job.status,
            job.provider_name,
            format!("{:?}", job.indicator_type),
            job.attempt_count,
            job.next_attempt_at.format("%Y-%m-%d %H:%M"),
            truncate_display(&job.indicator_value, 32),
            job.error_message.as_deref().unwrap_or("")
        );
    }
}

fn print_enrichment_providers(providers: &[db::EnrichmentProviderRecord]) {
    if providers.is_empty() {
        println!("No enrichment providers.");
        return;
    }

    println!(
        "{:<20} {:<14} {:<8} {:<10} Types",
        "Name", "Type", "Enabled", "Rate"
    );
    for provider in providers {
        let types = provider
            .supports_types
            .iter()
            .map(|indicator_type| format!("{indicator_type:?}"))
            .collect::<Vec<_>>()
            .join("|");
        println!(
            "{:<20} {:<14} {:<8} {:<10} {}",
            provider.name,
            provider.provider_type,
            if provider.enabled { "yes" } else { "no" },
            provider
                .rate_limit_per_minute
                .map(|limit| limit.to_string())
                .unwrap_or_else(|| "-".into()),
            types
        );
    }
}

fn import_cisa_kev_cache(source: &std::path::Path, data_dir: &std::path::Path) -> Result<()> {
    sentinel_enrichment::CisaKevProvider::from_json_file(source)
        .with_context(|| format!("validating CISA KEV cache: {}", source.display()))?;
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("creating data dir: {}", data_dir.display()))?;
    let target = cisa_kev_cache_path(data_dir);
    std::fs::copy(source, &target).with_context(|| {
        format!(
            "copying CISA KEV cache from {} to {}",
            source.display(),
            target.display()
        )
    })?;
    println!("Installed CISA KEV cache: {}", target.display());
    Ok(())
}

fn cisa_kev_cache_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("cisa-kev.json")
}

fn print_ioc_list(db: &db::Db, indicators: &[db::IndicatorRecord]) {
    if indicators.is_empty() {
        println!("No indicators.");
        return;
    }

    println!(
        "{:<6} {:<12} {:<14} {:<8} {:<20} Value",
        "ID", "Type", "Reputation", "Sight", "Last Seen"
    );
    for indicator in indicators {
        let reputation = db
            .get_latest_enrichment_results(indicator.id)
            .ok()
            .map(|results| ui::indicators::enrichment_reputation_label(&results))
            .unwrap_or_else(|| "Unknown".into());
        println!(
            "{:<6} {:<12} {:<14} {:<8} {:<20} {}",
            indicator.id,
            format!("{:?}", indicator.indicator_type),
            truncate_display(&reputation, 14),
            indicator.sighting_count,
            indicator.last_seen_at.format("%Y-%m-%d %H:%M"),
            indicator.normalized_value
        );
    }
}

fn print_ioc_show(db: &db::Db, value: &str) -> Result<()> {
    let mut matches = db.search_indicators(&db::IndicatorSearch {
        text: Some(value.to_string()),
        indicator_type: None,
        limit: Some(25),
    })?;
    matches.sort_by_key(|indicator| {
        if indicator.normalized_value.eq_ignore_ascii_case(value)
            || indicator.value.eq_ignore_ascii_case(value)
        {
            0
        } else {
            1
        }
    });

    let Some(indicator) = matches.first() else {
        println!("Indicator not found: {value}");
        return Ok(());
    };
    let Some(detail) = db.get_indicator_detail(indicator.id)? else {
        println!("Indicator not found: {value}");
        return Ok(());
    };
    let enrichment = db.get_latest_enrichment_results(indicator.id)?;
    print!("{}", format_ioc_detail(&detail, &enrichment));
    Ok(())
}

fn print_ioc_export(db: &db::Db, indicators: &[db::IndicatorRecord], format: &str) -> Result<()> {
    match format {
        "json" => {
            println!("{}", format_ioc_export_json(db, indicators)?);
            Ok(())
        }
        "csv" => {
            print!("{}", format_ioc_export_csv(db, indicators));
            Ok(())
        }
        _ => anyhow::bail!("unsupported IOC export format: {format}"),
    }
}

fn format_ioc_export_json(db: &db::Db, indicators: &[db::IndicatorRecord]) -> Result<String> {
    let rows = indicators
        .iter()
        .map(|indicator| {
            let enrichment = db
                .get_latest_enrichment_results(indicator.id)
                .unwrap_or_default();
            serde_json::json!({
                "id": indicator.id,
                "type": format!("{:?}", indicator.indicator_type),
                "value": indicator.value,
                "normalized_value": indicator.normalized_value,
                "reputation": ui::indicators::enrichment_reputation_label(&enrichment),
                "sighting_count": indicator.sighting_count,
                "first_seen_at": indicator.first_seen_at.to_rfc3339(),
                "last_seen_at": indicator.last_seen_at.to_rfc3339(),
                "confidence_score": indicator.confidence_score,
                "risk_score": indicator.risk_score,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&rows).map_err(Into::into)
}

fn format_ioc_export_csv(db: &db::Db, indicators: &[db::IndicatorRecord]) -> String {
    let mut output =
        "id,type,value,normalized_value,reputation,sighting_count,first_seen_at,last_seen_at\n"
            .to_string();
    for indicator in indicators {
        let enrichment = db
            .get_latest_enrichment_results(indicator.id)
            .unwrap_or_default();
        output.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            indicator.id,
            csv_escape(&format!("{:?}", indicator.indicator_type)),
            csv_escape(&indicator.value),
            csv_escape(&indicator.normalized_value),
            csv_escape(&ui::indicators::enrichment_reputation_label(&enrichment)),
            indicator.sighting_count,
            csv_escape(&indicator.first_seen_at.to_rfc3339()),
            csv_escape(&indicator.last_seen_at.to_rfc3339())
        ));
    }
    output
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn format_ioc_detail(
    detail: &db::IndicatorDetail,
    enrichment: &[db::EnrichmentResultRecord],
) -> String {
    let indicator = &detail.indicator;
    let mut output = String::new();
    output.push_str(&format!("Indicator: {}\n", indicator.normalized_value));
    output.push_str(&format!("Type: {:?}\n", indicator.indicator_type));
    output.push_str(&format!("Sightings: {}\n", indicator.sighting_count));
    output.push_str(&format!(
        "First Seen: {}\n",
        indicator.first_seen_at.format("%Y-%m-%d %H:%M:%S")
    ));
    output.push_str(&format!(
        "Last Seen: {}\n",
        indicator.last_seen_at.format("%Y-%m-%d %H:%M:%S")
    ));
    output.push_str(&format!(
        "Reputation: {}\n",
        ui::indicators::enrichment_reputation_label(enrichment)
    ));

    if !enrichment.is_empty() {
        output.push_str("\nEnrichment:\n");
        for result in enrichment {
            output.push_str(&format!(
                "- Provider #{}, status {}, score {}, verdict {}, summary {}\n",
                result.provider_id,
                result.status,
                result
                    .score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "-".into()),
                result.verdict.as_deref().unwrap_or("-"),
                result.summary.as_deref().unwrap_or("-")
            ));
        }
    }

    if !detail.occurrences.is_empty() {
        output.push_str("\nOccurrences:\n");
        for occurrence in detail.occurrences.iter().take(10) {
            output.push_str(&format!(
                "- alert {:?}, feed {:?}, field {}, offsets {}-{}, seen {}\n",
                occurrence.alert_id,
                occurrence.feed_id,
                occurrence.source_field.as_deref().unwrap_or("-"),
                occurrence
                    .start_offset
                    .map(|offset| offset.to_string())
                    .unwrap_or_else(|| "-".into()),
                occurrence
                    .end_offset
                    .map(|offset| offset.to_string())
                    .unwrap_or_else(|| "-".into()),
                occurrence.detected_at.format("%Y-%m-%d %H:%M:%S")
            ));
        }
    }

    output
}

fn print_extracted_iocs(source_name: &str, text: &str) {
    let input = sentinel_ioc::ExtractionInput {
        content_item_id: None,
        alert_id: None,
        feed_id: None,
        fields: vec![sentinel_ioc::ExtractionField {
            name: source_name,
            text,
        }],
    };
    let indicators = sentinel_ioc::extract_indicators(&input);
    print!("{}", format_extracted_indicators(&indicators));
}

fn format_extracted_indicators(indicators: &[sentinel_ioc::ExtractedIndicator]) -> String {
    if indicators.is_empty() {
        return "No indicators extracted.\n".into();
    }

    let mut output = String::new();
    output.push_str(&format!(
        "{:<12} {:<64} Source\n",
        "Type", "Normalized Value"
    ));
    for indicator in indicators {
        output.push_str(&format!(
            "{:<12} {:<64} {}:{}-{}\n",
            format!("{:?}", indicator.indicator_type),
            truncate_display(&indicator.normalized_value, 64),
            indicator.source_field,
            indicator.start_offset,
            indicator.end_offset
        ));
    }
    output
}

fn seed_demo_data(db: &db::Db) -> Result<()> {
    use crate::db::{AlertCreate, FeedCreate, KeywordCreate, TagCreate};
    use crate::types::{Criticality, FeedStatus, FeedType};

    // ── Feeds ───────────────────────────────────────────────────────────────
    let feed_ids = vec![
        db.create_feed(&FeedCreate {
            name: "Ransomfeed.it Tracker".into(),
            url: "https://api.ransomfeed.it/v1/posts".into(),
            feed_type: FeedType::Api,
            enabled: true,
            interval_secs: 300,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        })?,
        db.create_feed(&FeedCreate {
            name: "RansomLook Groups".into(),
            url: "https://api.ransomlook.io/v1/groups".into(),
            feed_type: FeedType::Api,
            enabled: true,
            interval_secs: 600,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        })?,
        db.create_feed(&FeedCreate {
            name: "BleepingComputer".into(),
            url: "https://www.bleepingcomputer.com/feed/".into(),
            feed_type: FeedType::Rss,
            enabled: true,
            interval_secs: 300,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        })?,
        db.create_feed(&FeedCreate {
            name: "SecurityWeek News".into(),
            url: "https://feeds.securityweek.com/securityweek".into(),
            feed_type: FeedType::Rss,
            enabled: true,
            interval_secs: 600,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        })?,
        db.create_feed(&FeedCreate {
            name: "CISA Alerts".into(),
            url: "https://www.cisa.gov/news-events/cybersecurity-advisories".into(),
            feed_type: FeedType::Website,
            enabled: true,
            interval_secs: 900,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: None,
        })?,
        db.create_feed(&FeedCreate {
            name: "Dark Web Monitor".into(),
            url: "http://ransomxifxwc5ste.onion/posts".into(),
            feed_type: FeedType::Onion,
            enabled: false,
            interval_secs: 1200,
            api_template_id: None,
            api_key: None,
            custom_headers: None,
            tor_proxy: Some("socks5h://127.0.0.1:9050".into()),
        })?,
    ];

    // ── Keywords ────────────────────────────────────────────────────────────
    let keyword_ids = vec![
        db.create_keyword(&KeywordCreate {
            pattern: "ransomware".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::Critical,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "CVE-[0-9]{4}-[0-9]+".into(),
            is_regex: true,
            case_sensitive: false,
            criticality: Criticality::High,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "APT[0-9]+".into(),
            is_regex: true,
            case_sensitive: false,
            criticality: Criticality::High,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "zero-day".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::Critical,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "phishing".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::Medium,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "malware".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::Medium,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "exploit".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::High,
            enabled: true,
        })?,
        db.create_keyword(&KeywordCreate {
            pattern: "backdoor".into(),
            is_regex: false,
            case_sensitive: false,
            criticality: Criticality::Critical,
            enabled: true,
        })?,
    ];

    // ── Alerts ──────────────────────────────────────────────────────────────
    let alerts = vec![
        (0, 0, "LockBit hits healthcare provider", "LockBit ransomware group claims attack on major healthcare provider, exfiltrating 2TB of patient data including medical records and insurance information.", Criticality::Critical),
        (0, 0, "BlackCat targets energy sector", "ALPHV/BlackCat ransomware operators have breached a European energy company, deploying encryption across Windows and Linux systems.", Criticality::Critical),
        (0, 6, "Ransomware exploit chain disclosed", "Security researchers disclose a new exploit chain used by ransomware groups leveraging CVE-2024-1234 for initial access.", Criticality::High),
        (1, 2, "APT29 targets diplomatic missions", "Cozy Bear (APT29) has been observed targeting diplomatic missions in Eastern Europe with sophisticated spear-phishing campaigns.", Criticality::High),
        (1, 2, "APT42 Iran-linked activity surge", "Mandiant reports increased activity from APT42, an Iran-nexus actor targeting journalists and academics with credential harvesting.", Criticality::High),
        (2, 1, "Critical CVE-2024-9876 in OpenSSL", "A critical buffer overflow vulnerability has been discovered in OpenSSL 3.0.x allowing remote code execution under specific configurations.", Criticality::High),
        (2, 3, "Chrome zero-day exploited in wild", "Google confirms active exploitation of a zero-day vulnerability (CVE-2024-8765) in Chrome. Patch immediately.", Criticality::Critical),
        (2, 4, "Phishing campaign targets banks", "A large-scale phishing campaign using brand impersonation of major banks has been detected, affecting users across 12 countries.", Criticality::Medium),
        (3, 5, "New InfoStealer malware family", "Researchers identify a new information stealer malware dubbed \"LummaC2\" being distributed via malicious Google Ads.", Criticality::Medium),
        (3, 7, "Supply chain backdoor discovered", "A backdoor has been discovered in a popular npm package downloaded over 2 million times weekly. The malicious code exfiltrates environment variables.", Criticality::Critical),
        (4, 6, "CISA adds CVE-2024-5432 to KEV catalog", "CISA has added CVE-2024-5432 to the Known Exploited Vulnerabilities catalog, requiring federal agencies to patch by January 30.", Criticality::High),
        (4, 3, "Zero-day in enterprise VPN appliances", "CISA warns of active exploitation of a zero-day vulnerability in widely deployed enterprise VPN appliances. No patch available yet.", Criticality::Critical),
        (4, 1, "Microsoft Patch Tuesday: 87 CVEs", "Microsoft releases January 2024 Patch Tuesday updates addressing 87 CVEs including 6 critical remote code execution vulnerabilities.", Criticality::Medium),
        (0, 7, "Ransomware deploys persistent backdoor", "Analysis reveals that recent ransomware deployments include a persistent backdoor mechanism ensuring re-entry even after remediation.", Criticality::Critical),
        (2, 5, "TrickBot malware resurfaces", "TrickBot malware infrastructure shows signs of reactivation with new command and control servers identified in Eastern Europe.", Criticality::Medium),
    ];

    for (i, (feed_idx, kw_idx, title, snippet, crit)) in alerts.iter().enumerate() {
        let hash = format!("demo-alert-hash-{:04}", i);
        db.create_alert(&AlertCreate {
            feed_id: feed_ids[*feed_idx],
            keyword_id: keyword_ids[*kw_idx],
            title: Some(title.to_string()),
            content_snippet: snippet.to_string(),
            criticality: *crit,
            content_hash: hash,
            metadata_json: None,
        })?;
    }

    // ── Tags ────────────────────────────────────────────────────────────────
    // Build a map of existing tag names to IDs so we can reuse built-in tags
    let existing_tags: std::collections::HashMap<String, i64> = db
        .list_tags()?
        .into_iter()
        .map(|tag| (tag.name.clone(), tag.id))
        .collect();

    let mut tag_ids = Vec::new();
    for (name, color, description) in [
        ("API", "#4CAF50", "REST API feeds"),
        ("News", "#FF9800", "General security news"),
        ("Government", "#9C27B0", "Government security sources"),
        ("Dark Web", "#333333", "Tor/onion sources"),
        ("Ransomware Gang", "#FF6B6B", "Dark web ransomware sources"),
    ] {
        let id = if let Some(&id) = existing_tags.get(name) {
            id
        } else {
            db.create_tag(&TagCreate {
                name: name.into(),
                color: color.into(),
                description: Some(description.into()),
            })?
        };
        tag_ids.push(id);
    }

    // ── Tag assignments ─────────────────────────────────────────────────────
    db.assign_tag_to_feed(feed_ids[0], tag_ids[0])?; // Ransomfeed -> API
    db.assign_tag_to_feed(feed_ids[0], tag_ids[4])?; // Ransomfeed -> Ransomware Gang
    db.assign_tag_to_feed(feed_ids[1], tag_ids[0])?; // RansomLook -> API
    db.assign_tag_to_feed(feed_ids[1], tag_ids[4])?; // RansomLook -> Ransomware Gang
    db.assign_tag_to_feed(feed_ids[2], tag_ids[1])?; // BleepingComputer -> News
    db.assign_tag_to_feed(feed_ids[3], tag_ids[1])?; // SecurityWeek -> News
    db.assign_tag_to_feed(feed_ids[4], tag_ids[2])?; // CISA -> Government
    db.assign_tag_to_feed(feed_ids[5], tag_ids[3])?; // Dark Web Monitor -> Dark Web

    db.assign_tag_to_keyword(keyword_ids[0], tag_ids[4])?; // ransomware -> Ransomware Gang
    db.assign_tag_to_keyword(keyword_ids[1], tag_ids[2])?; // CVE -> Government
    db.assign_tag_to_keyword(keyword_ids[3], tag_ids[2])?; // zero-day -> Government

    // ── Health logs ─────────────────────────────────────────────────────────
    db.add_health_log(feed_ids[0], FeedStatus::Healthy, None)?;
    db.add_health_log(feed_ids[1], FeedStatus::Healthy, None)?;
    db.add_health_log(feed_ids[2], FeedStatus::Healthy, None)?;
    db.add_health_log(
        feed_ids[3],
        FeedStatus::Warning,
        Some("RSS parse warning: unexpected element"),
    )?;
    db.add_health_log(feed_ids[4], FeedStatus::Healthy, None)?;
    db.add_health_log(
        feed_ids[5],
        FeedStatus::Disabled,
        Some("Feed disabled: Tor proxy unreachable"),
    )?;

    Ok(())
}

fn truncate_display(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        value.to_string()
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut app::App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(app.config.tick_rate_ms);
    let dashboard_refresh = Duration::from_secs(app.config.dashboard_refresh_secs);
    let mut last_tick = Instant::now();
    let mut last_dashboard_refresh = Instant::now();

    app.refresh_dashboard();

    while app.running {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if last_dashboard_refresh.elapsed() >= dashboard_refresh {
            app.refresh_dashboard();
            last_dashboard_refresh = Instant::now();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        cisa_kev_cache_path, csv_escape, format_extracted_indicators,
        format_feed_diagnostic_report, format_ioc_detail, format_ioc_export_csv, truncate_display,
    };
    use crate::db::{IndicatorDetail, IndicatorOccurrence, IndicatorRecord};
    use crate::feed::diagnostics::{
        FetchAttempt, FetchDiagnostic, FetchFailureKind, FetchFailurePhase,
    };
    use chrono::Utc;
    use sentinel_ioc::IndicatorType;
    use sentinel_ioc::{ExtractionField, ExtractionInput};

    #[test]
    fn format_extracted_indicators_shows_type_value_and_source() {
        let input = ExtractionInput {
            content_item_id: None,
            alert_id: None,
            feed_id: None,
            fields: vec![ExtractionField {
                name: "text",
                text: "Patch cve-2025-12345 at http://Bad.Example.NET/drop",
            }],
        };
        let indicators = sentinel_ioc::extract_indicators(&input);

        let output = format_extracted_indicators(&indicators);

        assert!(output.contains("Cve"));
        assert!(output.contains("CVE-2025-12345"));
        assert!(output.contains("http://bad.example.net/drop"));
        assert!(output.contains("text:6-20"));
    }

    #[test]
    fn truncate_display_handles_short_and_long_values() {
        assert_eq!(truncate_display("short", 10), "short");
        assert_eq!(truncate_display("abcdefghijkl", 5), "abcde...");
    }

    #[test]
    fn format_ioc_detail_includes_enrichment_and_occurrences() {
        let now = Utc::now();
        let detail = IndicatorDetail {
            indicator: IndicatorRecord {
                id: 1,
                indicator_type: IndicatorType::Cve,
                value: "cve-2025-12345".into(),
                normalized_value: "CVE-2025-12345".into(),
                first_seen_at: now,
                last_seen_at: now,
                sighting_count: 2,
                confidence_score: Some(90),
                risk_score: None,
                metadata_json: None,
                created_at: now,
                updated_at: now,
            },
            occurrences: vec![IndicatorOccurrence {
                id: 1,
                indicator_id: 1,
                content_item_id: None,
                alert_id: Some(7),
                feed_id: Some(3),
                source_field: Some("title".into()),
                start_offset: Some(4),
                end_offset: Some(18),
                surrounding_text: Some("See CVE-2025-12345".into()),
                detected_at: now,
            }],
        };
        let enrichment = vec![crate::db::EnrichmentResultRecord {
            id: 1,
            indicator_id: 1,
            provider_id: 2,
            status: "succeeded".into(),
            reputation: Some("Malicious".into()),
            score: Some(90),
            verdict: Some("Known Exploited".into()),
            summary: Some("CISA KEV match".into()),
            raw_json: None,
            fetched_at: now,
            expires_at: None,
            created_at: now,
            updated_at: now,
        }];

        let output = format_ioc_detail(&detail, &enrichment);

        assert!(output.contains("Indicator: CVE-2025-12345"));
        assert!(output.contains("Reputation: Known Exploited"));
        assert!(output.contains("CISA KEV match"));
        assert!(output.contains("alert Some(7)"));
    }

    #[test]
    fn csv_escape_quotes_values_when_needed() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("two,parts"), "\"two,parts\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn format_ioc_export_csv_includes_indicator_rows() {
        let db = crate::db::Db::new_in_memory_for_tests();
        db.init_schema().unwrap();
        let now = Utc::now();
        let indicators = vec![IndicatorRecord {
            id: 1,
            indicator_type: IndicatorType::Domain,
            value: "Bad.Example.NET".into(),
            normalized_value: "bad.example.net".into(),
            first_seen_at: now,
            last_seen_at: now,
            sighting_count: 3,
            confidence_score: Some(65),
            risk_score: None,
            metadata_json: None,
            created_at: now,
            updated_at: now,
        }];

        let output = format_ioc_export_csv(&db, &indicators);

        assert!(output.starts_with("id,type,value,normalized_value"));
        assert!(output.contains("1,Domain,Bad.Example.NET,bad.example.net,Unknown,3"));
    }

    #[test]
    fn format_feed_diagnostic_report_includes_failure_context() {
        let attempt = FetchAttempt {
            id: None,
            feed_id: Some(7),
            attempted_at: Some(Utc::now()),
            success: false,
            url: "https://example.test/feed.xml".into(),
            final_url: None,
            http_status: Some(404),
            elapsed_ms: 12,
            diagnostic: Some(FetchDiagnostic {
                phase: FetchFailurePhase::HttpStatus,
                kind: FetchFailureKind::HttpStatusClientError,
                summary: "Feed returned HTTP 404".into(),
                detail: Some("status code: 404".into()),
                http_status: Some(404),
                url: "https://example.test/feed.xml".into(),
                final_url: None,
                elapsed_ms: 12,
            }),
            items_seen: None,
            items_new: None,
        };

        let output = format_feed_diagnostic_report(Some("Example"), &attempt);

        assert!(output.contains("Feed: Example"));
        assert!(output.contains("Result: failed"));
        assert!(output.contains("HTTP status: 404"));
        assert!(output.contains("Phase: HTTP status"));
        assert!(output.contains("Kind: HTTP client error"));
        assert!(output.contains("Summary: Feed returned HTTP 404"));
    }

    #[test]
    fn cisa_kev_cache_path_uses_data_dir() {
        let path = cisa_kev_cache_path(std::path::Path::new("/tmp/threatdeck-data"));
        assert_eq!(
            path,
            std::path::Path::new("/tmp/threatdeck-data").join("cisa-kev.json")
        );
    }
}
