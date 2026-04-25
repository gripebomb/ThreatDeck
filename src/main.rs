#![allow(dead_code)]

mod ai;
mod alert;
mod app;
mod article;
mod config;
mod db;
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

    paths.ensure_dirs().context("creating config/data dirs")?;

    let app_config = config::load_app_config(&paths.config_file)?;
    let db = db::Db::open(&paths.db_file)?;
    db.init_schema().context("initializing database schema")?;

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
