#![allow(dead_code)]

mod ai;
mod alert;
mod auto_fetch;
mod app;
mod article;
mod config;
mod db;
mod enrichment;
mod feed;
mod keyword;
mod notify;
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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    res
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
        cisa_kev_cache_path, csv_escape, format_extracted_indicators, format_ioc_detail,
        format_ioc_export_csv, truncate_display,
    };
    use crate::db::{IndicatorDetail, IndicatorOccurrence, IndicatorRecord};
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
    fn cisa_kev_cache_path_uses_data_dir() {
        let path = cisa_kev_cache_path(std::path::Path::new("/tmp/threatdeck-data"));
        assert_eq!(
            path,
            std::path::Path::new("/tmp/threatdeck-data").join("cisa-kev.json")
        );
    }
}
