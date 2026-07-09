use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::auto_fetch::{AutoFetchMessage, AutoFetcher};
use crate::config::{AppConfig, Paths, TlsTrustStore};
use crate::db::{
    AlertFilter, Db, EnrichmentJobWithContext, EnrichmentProviderRecord, EnrichmentResultRecord,
    IndicatorRecord, IndicatorSearch,
};
use crate::theme::{get_runtime_theme, Theme};
use crate::types::*;
use crate::ui;
use crate::ui::alert_workbench::triage;
use crate::ui::alert_workbench::view_models::{
    AlertDetailViewModel, AlertListItem, AlertWorkbenchBundle, EnrichmentViewModel,
    IndicatorViewModel, TriageEventViewModel,
};
use crate::ui::alert_workbench::{
    AlertContextTab, AlertFilterState, AlertPane, AlertWorkbenchState,
};
use crate::ui::command_palette::{AppAction, CommandAction, CommandId, CommandPalette, ModalKind};
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Typing,
}

// ── Alert workbench app services ──────────────────────────────────────────────
//
// The app layer owns loading the split-pane alert workbench data. These free
// functions are the only consumers of storage for the workbench; the TUI
// renders the resulting view models and never issues SQL. See
// `tickets/02-workbench-view-models-and-data-bundle.md` and
// `docs/ARCHITECTURE.md` (Responsibility Boundaries).

/// Load the alert list as workbench view models.
pub fn load_alert_workbench_list(db: &Db, filter: &AlertFilterState) -> Result<Vec<AlertListItem>> {
    let db_filter = AlertFilter {
        text: if filter.text.is_empty() {
            None
        } else {
            Some(filter.text.clone())
        },
        criticality: filter.severity,
        unread_only: filter.unread_only,
        status: filter.status,
        disposition: filter.disposition,
        open_only: filter.hide_closed,
        limit: Some(500),
        ..AlertFilter::default()
    };
    let rows = db.list_alerts(&db_filter)?;
    Ok(rows.iter().map(AlertListItem::from).collect())
}

/// Load the full selected-alert bundle: details plus indicators (with nested
/// enrichment), metadata, and triage history.
///
/// Returns `Ok(None)` when the alert no longer exists (e.g. deleted before the
/// load completed). Missing optional data (no IOCs, no history) yields a bundle
/// with empty lists, not an error.
pub fn load_alert_workbench_bundle(db: &Db, alert_id: i64) -> Result<Option<AlertWorkbenchBundle>> {
    let alert = match db.get_alert(alert_id)? {
        Some(a) => a,
        None => return Ok(None),
    };

    let feed = db.get_feed(alert.feed_id)?;
    let keyword = db.get_keyword(alert.keyword_id)?;
    let tags = db.get_alert_tags(alert.id)?;

    let tag_names = tags.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
    let feed_name = feed
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "(unknown feed)".to_string());
    let feed_url = feed.as_ref().map(|f| f.url.clone());
    let keyword_pattern = keyword
        .as_ref()
        .map(|k| k.pattern.clone())
        .unwrap_or_else(|| "(unknown keyword)".to_string());

    let detail =
        AlertDetailViewModel::from_parts(&alert, feed_name, feed_url, keyword_pattern, tag_names);

    // Indicators, each with its latest enrichment results nested. Fetch the
    // enrichment for all indicators in a single batched query (one round trip)
    // instead of one query per indicator (N+1).
    let indicator_records = db.list_indicators_for_alert(alert.id)?;
    let indicator_ids: Vec<i64> = indicator_records.iter().map(|r| r.id).collect();
    let mut indicators: Vec<IndicatorViewModel> =
        indicator_records.iter().map(IndicatorViewModel::from).collect();
    let enrichment_by_indicator =
        db.get_latest_enrichment_results_for_indicators(&indicator_ids)?;
    for indicator in &mut indicators {
        let enrichment = enrichment_by_indicator
            .get(&indicator.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        indicator.enrichment = enrichment.iter().map(EnrichmentViewModel::from).collect();
    }

    let triage_history = db
        .list_alert_triage_events(alert.id)?
        .iter()
        .map(TriageEventViewModel::from)
        .collect();

    // Raw feed-item content is optional and currently unresolvable: alerts
    // carry no direct link back to their originating feed item.
    Ok(Some(AlertWorkbenchBundle {
        detail: Some(detail),
        indicators,
        metadata_json: alert.metadata_json.clone(),
        triage_history,
        raw_content: None,
    }))
}

// Storage-row → view-model conversions live in the app layer so the TUI never
// imports storage row types. (Both sides are local to this crate, so these
// `From` impls are permitted here under the orphan rules.)
impl From<&IndicatorRecord> for IndicatorViewModel {
    fn from(value: &IndicatorRecord) -> Self {
        Self {
            id: value.id,
            indicator_type: value.indicator_type,
            value: value.value.clone(),
            normalized_value: value.normalized_value.clone(),
            sighting_count: value.sighting_count,
            confidence: value.confidence_score,
            risk: value.risk_score,
            // Enrichment is attached by the loader after construction.
            enrichment: Vec::new(),
        }
    }
}

impl From<&EnrichmentResultRecord> for EnrichmentViewModel {
    fn from(value: &EnrichmentResultRecord) -> Self {
        Self {
            provider_id: value.provider_id,
            status: value.status.clone(),
            reputation: value.reputation.clone(),
            score: value.score,
            verdict: value.verdict.clone(),
            summary: value.summary.clone(),
            fetched_at: value.fetched_at,
        }
    }
}

impl From<&crate::db::AlertTriageEvent> for TriageEventViewModel {
    fn from(value: &crate::db::AlertTriageEvent) -> Self {
        Self {
            id: value.id,
            event_type: value.event_type.clone(),
            old_value: value.old_value.clone(),
            new_value: value.new_value.clone(),
            note: value.note.clone(),
            actor: value.actor.clone(),
            created_at: value.created_at,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub prev_screen: Option<Screen>,
    pub db: Db,
    pub config: AppConfig,
    pub paths: Paths,
    pub theme: &'static Theme,
    pub running: bool,
    pub notification: Option<(String, NotificationType)>,
    pub notification_at: Option<Instant>,
    pub show_help: bool,
    pub show_confirm: Option<ConfirmDialog>,
    pub form_focus: usize,
    pub input_mode: InputMode,
    pub filter_active: bool,
    pub pending_g: bool,

    // Dashboard
    pub dashboard_stats: Stats,
    pub dashboard_recent_alerts: Vec<AlertWithMeta>,
    pub dashboard_criticality_data: Vec<(Criticality, i64)>,
    pub dashboard_status_counts: std::collections::HashMap<String, i64>,
    pub dashboard_disposition_counts: std::collections::HashMap<String, i64>,

    // Feeds
    pub feeds_list: Vec<FeedWithTags>,
    pub feeds_selected: usize,
    pub feeds_filter: String,
    pub feeds_show_form: bool,
    pub feeds_form: FeedForm,
    pub feeds_form_edit_id: Option<i64>,
    pub feeds_detail_view: bool,
    pub feeds_sort: usize,
    pub feeds_selected_attempts: Vec<crate::feed::diagnostics::FetchAttempt>,

    // Alerts
    pub alerts_list: Vec<AlertWithMeta>,
    pub alerts_selected: usize,
    pub alerts_filter: String,
    pub alerts_filter_criticality: Option<Criticality>,
    pub alerts_filter_unread_only: bool,
    pub alerts_filter_status: Option<AlertStatus>,
    pub alerts_filter_disposition: Option<AlertDisposition>,
    pub alerts_hide_closed: bool,
    pub alerts_detail_view: bool,
    pub alerts_bulk_mode: bool,
    pub alerts_selected_bulk: HashSet<i64>,
    pub triage_history_view: bool,
    pub triage_enum_select_mode: bool,
    pub triage_enum_target: Option<TriageEnumTarget>,
    pub triage_enum_selected: usize,
    pub triage_note_input_mode: bool,
    pub triage_note_input: String,
    pub triage_input_target: crate::types::TriageInputTarget,

    // Alert workbench (split-pane view)
    pub workbench: AlertWorkbenchState,
    pub workbench_items: Vec<AlertListItem>,
    pub workbench_bundle: Option<AlertWorkbenchBundle>,

    // Articles
    pub articles_list: Vec<FeedItemWithFeed>,
    pub articles_selected: usize,
    pub articles_filter: String,
    pub articles_unread_only: bool,
    pub articles_reader: bool,
    pub articles_scroll: u16,

    // Indicators
    pub indicators_list: Vec<IndicatorRecord>,
    pub indicators_selected: usize,
    pub indicators_filter: String,
    pub indicators_filter_type: Option<sentinel_ioc::IndicatorType>,

    // Enrichment queue
    pub enrichment_queue_list: Vec<EnrichmentJobWithContext>,
    pub enrichment_queue_selected: usize,
    pub enrichment_queue_filter: String,

    // Keywords
    pub keywords_list: Vec<Keyword>,
    pub keyword_tags: HashMap<i64, Vec<Tag>>,
    pub keywords_selected: usize,
    pub keywords_filter: String,
    pub keywords_show_form: bool,
    pub keywords_form: KeywordForm,
    pub keywords_form_edit_id: Option<i64>,
    pub keywords_test_mode: bool,
    pub keywords_test_input: String,
    pub keywords_test_results: Vec<MatchResult>,

    // Tags
    pub tags_list: Vec<Tag>,
    pub tag_usage_counts: HashMap<i64, i64>,
    pub tags_selected: usize,
    pub tags_filter: String,
    pub tags_show_form: bool,
    pub tags_form: TagForm,
    pub tags_form_edit_id: Option<i64>,
    pub tags_assignment_mode: bool,
    pub tags_assignment_target: Option<TagAssignmentTarget>,

    // Logs
    pub logs_list: Vec<FeedHealthLog>,
    pub logs_selected: usize,
    pub logs_filter_feed: Option<i64>,
    pub logs_filter: String,

    // Settings
    pub settings_tab: SettingsTab,
    pub settings_retention_days: u32,
    pub settings_theme_name: String,
    pub settings_notifications: Vec<NotificationConfig>,
    pub settings_enrichment_providers: Vec<EnrichmentProviderRecord>,
    pub settings_enrichment_provider_selected: usize,
    pub settings_notif_form: bool,
    pub settings_notif_form_data: NotificationForm,
    pub settings_notif_form_edit_id: Option<i64>,
    pub settings_cleanup_preview: Option<u64>,

    pub auto_fetcher: Option<AutoFetcher>,
    pub auto_fetch_rx: Option<mpsc::Receiver<AutoFetchMessage>>,
    pub settings_tls_trust_store: TlsTrustStore,
    pub settings_auto_fetch_enabled: bool,
    pub settings_auto_fetch_interval: u32,
    pub command_palette: CommandPalette,
}

impl App {
    pub fn new(db: Db, config: AppConfig, paths: Paths) -> Self {
        let theme = get_runtime_theme(&config.theme);
        let mut app = Self {
            screen: Screen::Dashboard,
            prev_screen: None,
            db,
            theme,
            config,
            paths,
            running: true,
            notification: None,
            notification_at: None,
            show_help: false,
            show_confirm: None,
            form_focus: 0,
            input_mode: InputMode::Normal,
            filter_active: false,
            pending_g: false,
            dashboard_stats: Stats::default(),
            dashboard_recent_alerts: Vec::new(),
            dashboard_criticality_data: Vec::new(),
            dashboard_status_counts: std::collections::HashMap::new(),
            dashboard_disposition_counts: std::collections::HashMap::new(),
            feeds_list: Vec::new(),
            feeds_selected: 0,
            feeds_filter: String::new(),
            feeds_show_form: false,
            feeds_form: FeedForm::default(),
            feeds_form_edit_id: None,
            feeds_detail_view: false,
            feeds_sort: 0,
            feeds_selected_attempts: Vec::new(),
            alerts_list: Vec::new(),
            alerts_selected: 0,
            alerts_filter: String::new(),
            alerts_filter_criticality: None,
            alerts_filter_unread_only: false,
            alerts_filter_status: None,
            alerts_filter_disposition: None,
            alerts_hide_closed: true,
            alerts_detail_view: false,
            alerts_bulk_mode: false,
            alerts_selected_bulk: HashSet::new(),
            triage_history_view: false,
            triage_enum_select_mode: false,
            triage_enum_target: None,
            triage_enum_selected: 0,
            triage_note_input_mode: false,
            triage_note_input: String::new(),
            triage_input_target: crate::types::TriageInputTarget::default(),
            workbench: AlertWorkbenchState::new(),
            workbench_items: Vec::new(),
            workbench_bundle: None,
            articles_list: Vec::new(),
            articles_selected: 0,
            articles_filter: String::new(),
            articles_unread_only: false,
            articles_reader: false,
            articles_scroll: 0,
            indicators_list: Vec::new(),
            indicators_selected: 0,
            indicators_filter: String::new(),
            indicators_filter_type: None,
            enrichment_queue_list: Vec::new(),
            enrichment_queue_selected: 0,
            enrichment_queue_filter: String::new(),
            keywords_list: Vec::new(),
            keyword_tags: HashMap::new(),
            keywords_selected: 0,
            keywords_filter: String::new(),
            keywords_show_form: false,
            keywords_form: KeywordForm::default(),
            keywords_form_edit_id: None,
            keywords_test_mode: false,
            keywords_test_input: String::new(),
            keywords_test_results: Vec::new(),
            tags_list: Vec::new(),
            tag_usage_counts: HashMap::new(),
            tags_selected: 0,
            tags_filter: String::new(),
            tags_show_form: false,
            tags_form: TagForm::default(),
            tags_form_edit_id: None,
            tags_assignment_mode: false,
            tags_assignment_target: None,
            logs_list: Vec::new(),
            logs_selected: 0,
            logs_filter_feed: None,
            logs_filter: String::new(),
            settings_tab: SettingsTab::General,
            settings_retention_days: 30,
            settings_theme_name: "dark".to_string(),
            settings_notifications: Vec::new(),
            settings_enrichment_providers: Vec::new(),
            settings_enrichment_provider_selected: 0,
            settings_notif_form: false,
            settings_notif_form_data: NotificationForm::default(),
            settings_notif_form_edit_id: None,
            settings_cleanup_preview: None,
            auto_fetcher: None,
            auto_fetch_rx: None,
            settings_tls_trust_store: TlsTrustStore::Bundled,
            settings_auto_fetch_enabled: false,
            settings_auto_fetch_interval: 30,
            command_palette: CommandPalette::new(),
        };
        app.refresh_dashboard();
        app.refresh_feeds();
        app.refresh_alerts();
        app.refresh_workbench();
        app.refresh_articles();
        app.refresh_indicators();
        app.refresh_enrichment_queue();
        app.refresh_keywords();
        app.refresh_tags();
        app.refresh_logs();
        app.refresh_settings();
        app.settings_auto_fetch_enabled = app.config.auto_fetch.enabled;
        app.settings_auto_fetch_interval = app.config.auto_fetch.interval_minutes;
        app.settings_tls_trust_store = app.config.network.tls_trust_store;

        if app.config.auto_fetch.enabled {
            let (tx, rx) = mpsc::channel();
            let fetcher = match AutoFetcher::spawn(
                app.paths.db_file.clone(),
                app.config.auto_fetch.interval_minutes,
                app.config.network.tls_trust_store,
                tx,
            ) {
                Ok(fetcher) => fetcher,
                Err(e) => {
                    eprintln!("Failed to start auto-fetcher: {}", e);
                    return app;
                }
            };
            app.auto_fetcher = Some(fetcher);
            app.auto_fetch_rx = Some(rx);
        }
        app
    }

    pub fn on_tick(&mut self) {
        if self
            .notification_at
            .map(|shown_at| shown_at.elapsed().as_secs() >= 4)
            .unwrap_or(false)
        {
            self.clear_notification();
        }

        if let Some(rx) = &self.auto_fetch_rx {
            if let Ok(msg) = rx.try_recv() {
                match msg {
                    AutoFetchMessage::Completed {
                        feeds_attempted,
                        feeds_succeeded,
                        alerts_created,
                        errors,
                    } => {
                        self.refresh_dashboard();
                        self.refresh_alerts();
                        self.refresh_workbench();
                        self.refresh_articles();
                        self.refresh_indicators();
                        self.refresh_feeds();
                        self.refresh_logs();

                        let mut msg_text = format!(
                            "Auto-fetched {}/{} feed(s), created {} alert(s)",
                            feeds_succeeded, feeds_attempted, alerts_created
                        );
                        let notif_type = if errors.is_empty() {
                            crate::types::NotificationType::Success
                        } else {
                            msg_text
                                .push_str(&format!(" ({} error(s) — check logs)", errors.len()));
                            crate::types::NotificationType::Warning
                        };
                        self.set_notification(msg_text, notif_type);
                    }
                    AutoFetchMessage::Stopped => {
                        self.set_notification(
                            "Auto-fetch stopped".to_string(),
                            crate::types::NotificationType::Info,
                        );
                    }
                }
            }
        }
    }

    pub fn set_notification(&mut self, msg: String, typ: NotificationType) {
        self.notification = Some((msg, typ));
        self.notification_at = Some(Instant::now());
    }

    pub fn clear_notification(&mut self) {
        self.notification = None;
        self.notification_at = None;
    }

    pub fn switch_screen(&mut self, screen: Screen) {
        self.prev_screen = Some(self.screen);
        self.screen = screen;
        self.show_help = false;
        self.show_confirm = None;
        self.form_focus = 0;
        self.input_mode = InputMode::Normal;
        self.filter_active = false;
        self.pending_g = false;
        match screen {
            Screen::Dashboard => self.refresh_dashboard(),
            Screen::Feeds => self.refresh_feeds(),
            Screen::Alerts => self.refresh_workbench(),
            Screen::Articles => self.refresh_articles(),
            Screen::Indicators => self.refresh_indicators(),
            Screen::EnrichmentQueue => self.refresh_enrichment_queue(),
            Screen::Keywords => self.refresh_keywords(),
            Screen::Tags => self.refresh_tags(),
            Screen::Logs => self.refresh_logs(),
            Screen::Settings => self.refresh_settings(),
        }
    }

    pub fn go_back(&mut self) {
        if let Some(prev) = self.prev_screen.take() {
            self.screen = prev;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.command_palette.state.is_open {
            self.handle_palette_key(key);
            return;
        }

        if self.show_help {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('?')
                || key.code == KeyCode::F(1)
            {
                self.show_help = false;
            }
            return;
        }

        if self.show_confirm.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_action(),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.show_confirm = None,
                _ => {}
            }
            return;
        }

        if self.filter_active {
            self.handle_filter_key(key);
            return;
        }

        if self.tags_assignment_mode {
            self.handle_tag_assignment_key(key);
            return;
        }

        if self.input_mode == InputMode::Typing {
            if key.code == KeyCode::Esc {
                self.input_mode = InputMode::Normal;
                // Cancel any in-flight triage input (note/owner entry or enum
                // selector) so Esc cleanly exits it (Ticket 09).
                self.triage_note_input_mode = false;
                self.triage_note_input.clear();
                self.triage_enum_select_mode = false;
                self.triage_enum_target = None;
                self.triage_enum_selected = 0;
                return;
            }
            self.handle_screen_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') if !self.in_form() => self.running = false,
            KeyCode::Char('1') => self.switch_screen(Screen::Dashboard),
            KeyCode::Char('2') => self.switch_screen(Screen::Feeds),
            KeyCode::Char('3') => self.switch_screen(Screen::Alerts),
            KeyCode::Char('4') => self.switch_screen(Screen::Articles),
            KeyCode::Char('5') => self.switch_screen(Screen::Indicators),
            KeyCode::Char('6') => self.switch_screen(Screen::EnrichmentQueue),
            KeyCode::Char('7') => self.switch_screen(Screen::Keywords),
            KeyCode::Char('8') => self.switch_screen(Screen::Tags),
            KeyCode::Char('9') => self.switch_screen(Screen::Logs),
            KeyCode::Char('0') => self.switch_screen(Screen::Settings),
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,
            KeyCode::Char('/') => self.start_filter(),
            KeyCode::Char(':') => {
                let ctx = self.command_context();
                self.command_palette.state.open_colon(&ctx);
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let ctx = self.command_context();
                self.command_palette.state.open_fuzzy(&ctx);
            }
            KeyCode::Esc => self.handle_esc(),
            _ => self.handle_screen_key(key),
        }
    }

    fn handle_palette_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.command_palette.state.close();
            }
            KeyCode::Enter => {
                if let Some(cmd) = self.command_palette.state.selected_command() {
                    let cmd_id = cmd.id;
                    let action = cmd.action;
                    self.command_palette.state.close();
                    self.execute_command(cmd_id, action);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.command_palette.state.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.command_palette.state.move_down();
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.command_palette.state.move_up();
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.command_palette.state.move_down();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.command_palette.state.clear_input();
            }
            KeyCode::Backspace => {
                self.command_palette.state.backspace();
            }
            KeyCode::Char(c) => {
                self.command_palette.state.input_char(c);
            }
            _ => {}
        }
    }

    /// Build the command-palette availability context from the current screen
    /// and selections, so the palette can hide commands that don't apply here
    /// (e.g. workbench-only actions off the Alerts screen, or selection-based
    /// actions with nothing selected).
    fn command_context(&self) -> crate::ui::command_palette::registry::CommandContext {
        let (discord, webhook, email) = (
            self.settings_notifications
                .iter()
                .any(|n| n.enabled && matches!(n.channel, NotificationChannel::Discord)),
            self.settings_notifications
                .iter()
                .any(|n| n.enabled && matches!(n.channel, NotificationChannel::Webhook)),
            self.settings_notifications
                .iter()
                .any(|n| n.enabled && matches!(n.channel, NotificationChannel::Email)),
        );
        crate::ui::command_palette::registry::CommandContext {
            current_screen: self.screen,
            has_selected_feed: self.feeds_list.get(self.feeds_selected).is_some(),
            has_selected_alert: self.workbench.selected_alert_id().is_some(),
            has_selected_keyword: self.keywords_list.get(self.keywords_selected).is_some(),
            discord_configured: discord,
            webhook_configured: webhook,
            email_configured: email,
        }
    }

    fn execute_command(&mut self, _cmd_id: CommandId, action: CommandAction) {
        match action {
            CommandAction::Navigate(screen) => {
                self.switch_screen(screen);
                self.set_notification(
                    format!("Opened {}", screen),
                    crate::types::NotificationType::Info,
                );
            }
            CommandAction::OpenModal(modal) => {
                if modal == ModalKind::Help {
                    self.show_help = true;
                }
            }
            CommandAction::Dispatch(app_action) => {
                self.handle_app_action(app_action);
            }
            CommandAction::Quit => {
                self.running = false;
            }
        }
    }

    // ── Workbench palette actions (Ticket 10) ─────────────────────────────

    /// Switch to the Alerts workbench and set pane focus.
    fn focus_workbench_pane(&mut self, pane: AlertPane, label: &str) {
        self.switch_screen(Screen::Alerts);
        self.workbench.focused_pane = pane;
        self.set_notification(
            format!("Focus: {label}"),
            crate::types::NotificationType::Info,
        );
    }

    /// Switch to the Alerts workbench and select a context tab.
    fn select_workbench_tab(&mut self, tab: AlertContextTab, label: &str) {
        self.switch_screen(Screen::Alerts);
        self.workbench.bottom_tab = tab;
        self.workbench.reset_context_scroll();
        self.set_notification(
            format!("Tab: {label}"),
            crate::types::NotificationType::Info,
        );
    }

    /// Switch to the Alerts workbench and run a one-shot triage action on the
    /// selected alert. Warns if nothing is selected.
    fn run_workbench_triage(&mut self, action: fn(&mut App)) {
        self.switch_screen(Screen::Alerts);
        if self.workbench.selected_alert_id().is_some() {
            action(self);
        } else {
            self.set_notification(
                "No alert selected".to_string(),
                crate::types::NotificationType::Warning,
            );
        }
    }

    fn handle_app_action(&mut self, action: AppAction) {
        match action {
            AppAction::Refresh => {
                match self.screen {
                    Screen::Dashboard => self.refresh_dashboard(),
                    Screen::Feeds => self.refresh_feeds(),
                    Screen::Alerts => self.refresh_workbench(),
                    Screen::Articles => self.refresh_articles(),
                    Screen::Indicators => self.refresh_indicators(),
                    Screen::EnrichmentQueue => self.refresh_enrichment_queue(),
                    Screen::Keywords => self.refresh_keywords(),
                    Screen::Tags => self.refresh_tags(),
                    Screen::Logs => self.refresh_logs(),
                    Screen::Settings => self.refresh_settings(),
                }
                self.set_notification(
                    "Refreshed".to_string(),
                    crate::types::NotificationType::Success,
                );
            }
            AppAction::FeedAdd => {
                self.open_feed_add_form();
            }
            AppAction::FeedEditSelected => {
                if let Some(ft) = self.feeds_list.get(self.feeds_selected) {
                    let f = &ft.feed;
                    self.feeds_form = crate::types::FeedForm {
                        name: f.name.clone(),
                        url: f.url.clone(),
                        feed_type: f.feed_type,
                        interval_secs: f.interval_secs,
                        enabled: f.enabled,
                        api_template_id: f.api_template_id,
                        api_key: f.api_key.clone().unwrap_or_default(),
                        custom_headers: f.custom_headers.clone().unwrap_or_default(),
                        tor_proxy: f.tor_proxy.clone().unwrap_or_default(),
                    };
                    self.feeds_form_edit_id = Some(f.id);
                    self.feeds_show_form = true;
                    self.form_focus = 0;
                    self.input_mode = InputMode::Normal;
                    self.set_notification(
                        "Edit feed form opened".to_string(),
                        crate::types::NotificationType::Info,
                    );
                } else {
                    self.set_notification(
                        "No feed selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::FeedCheckSelected => {
                if self.feeds_list.is_empty() {
                    self.set_notification(
                        "No feed selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                } else {
                    self.fetch_selected_feed();
                }
            }
            AppAction::FeedCheckAll => {
                self.set_notification(
                    "Checking all enabled feeds...".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
            AppAction::FeedEnableSelected => {
                if let Some(feed_id) = self
                    .feeds_list
                    .get(self.feeds_selected)
                    .map(|ft| ft.feed.id)
                {
                    let _ = self.db.toggle_feed_enabled(feed_id);
                    self.refresh_feeds();
                    if let Some(ft) = self.feeds_list.get(self.feeds_selected) {
                        self.set_notification(
                            format!("Feed '{}' enabled", ft.feed.name),
                            crate::types::NotificationType::Success,
                        );
                    }
                } else {
                    self.set_notification(
                        "No feed selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::FeedDisableSelected => {
                if let Some(feed_id) = self
                    .feeds_list
                    .get(self.feeds_selected)
                    .map(|ft| ft.feed.id)
                {
                    let _ = self.db.toggle_feed_enabled(feed_id);
                    self.refresh_feeds();
                    if let Some(ft) = self.feeds_list.get(self.feeds_selected) {
                        self.set_notification(
                            format!("Feed '{}' disabled", ft.feed.name),
                            crate::types::NotificationType::Success,
                        );
                    }
                } else {
                    self.set_notification(
                        "No feed selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::AlertShowUnread => {
                self.alerts_filter_unread_only = true;
                self.switch_screen(Screen::Alerts);
                self.set_notification(
                    "Showing unread alerts".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
            AppAction::AlertShowCritical => {
                self.alerts_filter_criticality = Some(crate::types::Criticality::Critical);
                self.switch_screen(Screen::Alerts);
                self.set_notification(
                    "Showing critical alerts".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
            AppAction::AlertMarkSelectedRead => {
                if let Some(id) = self.workbench.selected_alert_id() {
                    let _ = self.db.mark_alert_read(id, true);
                    self.refresh_workbench();
                    self.refresh_dashboard();
                    self.set_notification(
                        "Alert marked as read".to_string(),
                        crate::types::NotificationType::Success,
                    );
                } else {
                    self.set_notification(
                        "No alert selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::AlertMarkSelectedUnread => {
                if let Some(id) = self.workbench.selected_alert_id() {
                    let _ = self.db.mark_alert_read(id, false);
                    self.refresh_workbench();
                    self.refresh_dashboard();
                    self.set_notification(
                        "Alert marked as unread".to_string(),
                        crate::types::NotificationType::Success,
                    );
                } else {
                    self.set_notification(
                        "No alert selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::AlertMarkVisibleRead => {
                let _ = self.db.mark_all_alerts_read(true);
                self.refresh_workbench();
                self.refresh_dashboard();
                self.set_notification(
                    "All alerts marked as read".to_string(),
                    crate::types::NotificationType::Success,
                );
            }
            AppAction::AlertExportSelectedMarkdown => {
                if self.workbench.selected_alert_id().is_some() {
                    // Reuse the workbench exporter (operates on the selected
                    // alert and notifies with the path / error).
                    crate::ui::alert_workbench::triage::export_selected(self);
                } else {
                    self.set_notification(
                        "No alert selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::AlertExportVisibleMarkdown => {
                let report_service = crate::report::ReportService::new();
                let filter = AlertFilter {
                    text: if self.alerts_filter.is_empty() {
                        None
                    } else {
                        Some(self.alerts_filter.clone())
                    },
                    criticality: self.alerts_filter_criticality,
                    unread_only: self.alerts_filter_unread_only,
                    status: self.alerts_filter_status,
                    disposition: self.alerts_filter_disposition,
                    open_only: self.alerts_hide_closed,
                    limit: Some(500),
                    ..AlertFilter::default()
                };
                let options = threatdeck_report::ReportExportOptions {
                    report_type: threatdeck_report::ReportType::AlertCollection,
                    format: threatdeck_report::ExportFormat::Markdown,
                    output_path: None,
                    include_raw_content: false,
                    include_metadata: true,
                    include_iocs: true,
                    include_enrichment: true,
                    include_triage_history: true,
                    include_feed_health: false,
                    include_tags: true,
                    redact_secrets: true,
                    overwrite: false,
                    generated_by: None,
                };
                let export_dir = self.paths.data_dir.join("exports");
                // Re-query with the workbench's current filter so the export
                // reflects what the user sees (alerts_list is the legacy vec).
                let alerts = match self.db.list_alerts(&filter) {
                    Ok(a) => a,
                    Err(e) => {
                        self.set_notification(
                            format!("Export failed: {}", e),
                            crate::types::NotificationType::Error,
                        );
                        return;
                    }
                };
                let count = alerts.len();
                match report_service.export_visible_alerts_report(
                    &self.db,
                    &alerts,
                    &filter,
                    &options,
                    &export_dir,
                ) {
                    Ok(result) => {
                        self.set_notification(
                            format!("Exported {} alerts: {}", count, result.path.display()),
                            crate::types::NotificationType::Success,
                        );
                    }
                    Err(e) => {
                        self.set_notification(
                            format!("Export failed: {}", e),
                            crate::types::NotificationType::Error,
                        );
                    }
                }
            }
            // ── Workbench navigation + triage (Ticket 10) ────────────────────────
            AppAction::AlertFocusList => {
                self.focus_workbench_pane(AlertPane::AlertList, "alert list")
            }
            AppAction::AlertFocusDetails => {
                self.focus_workbench_pane(AlertPane::AlertDetails, "details")
            }
            AppAction::AlertFocusContext => {
                self.focus_workbench_pane(AlertPane::ContextPanel, "context")
            }
            AppAction::AlertTabIndicators => {
                self.select_workbench_tab(AlertContextTab::Indicators, "Indicators")
            }
            AppAction::AlertTabMetadata => {
                self.select_workbench_tab(AlertContextTab::Metadata, "Metadata")
            }
            AppAction::AlertTabEnrichment => {
                self.select_workbench_tab(AlertContextTab::Enrichment, "Enrichment")
            }
            AppAction::AlertTabHistory => {
                self.select_workbench_tab(AlertContextTab::TriageHistory, "Triage history")
            }
            AppAction::AlertTabRaw => {
                self.select_workbench_tab(AlertContextTab::RawContent, "Raw content")
            }
            AppAction::AlertAcknowledge => self.run_workbench_triage(triage::acknowledge),
            AppAction::AlertInvestigate => self.run_workbench_triage(triage::investigate),
            AppAction::AlertEscalate => self.run_workbench_triage(triage::escalate),
            AppAction::AlertClose => self.run_workbench_triage(triage::close),
            AppAction::AlertReopen => self.run_workbench_triage(triage::reopen),
            AppAction::FeedHealthExportMarkdown => {
                let report_service = crate::report::ReportService::new();
                let options = threatdeck_report::ReportExportOptions {
                    report_type: threatdeck_report::ReportType::FeedHealth,
                    format: threatdeck_report::ExportFormat::Markdown,
                    output_path: None,
                    include_raw_content: false,
                    include_metadata: true,
                    include_iocs: false,
                    include_enrichment: false,
                    include_triage_history: false,
                    include_feed_health: true,
                    include_tags: false,
                    redact_secrets: true,
                    overwrite: false,
                    generated_by: None,
                };
                let export_dir = self.paths.data_dir.join("exports");
                match report_service.export_feed_health_report(&self.db, &options, &export_dir) {
                    Ok(result) => {
                        self.set_notification(
                            format!("Exported feed health: {}", result.path.display()),
                            crate::types::NotificationType::Success,
                        );
                    }
                    Err(e) => {
                        self.set_notification(
                            format!("Export failed: {}", e),
                            crate::types::NotificationType::Error,
                        );
                    }
                }
            }
            AppAction::KeywordAdd => {
                self.open_keyword_add_form();
            }
            AppAction::KeywordEditSelected => {
                if let Some(k) = self.keywords_list.get(self.keywords_selected) {
                    self.keywords_form = crate::types::KeywordForm {
                        pattern: k.pattern.clone(),
                        is_regex: k.is_regex,
                        case_sensitive: k.case_sensitive,
                        criticality: k.criticality,
                        enabled: k.enabled,
                    };
                    self.keywords_form_edit_id = Some(k.id);
                    self.keywords_show_form = true;
                    self.form_focus = 0;
                    self.input_mode = InputMode::Normal;
                    self.set_notification(
                        "Edit keyword form opened".to_string(),
                        crate::types::NotificationType::Info,
                    );
                } else {
                    self.set_notification(
                        "No keyword selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::KeywordTestSelected => {
                if let Some(k) = self.keywords_list.get(self.keywords_selected) {
                    self.keywords_test_mode = true;
                    self.keywords_test_input = String::new();
                    self.keywords_test_results = Vec::new();
                    self.set_notification(
                        format!("Testing keyword: {}", k.pattern),
                        crate::types::NotificationType::Info,
                    );
                } else {
                    self.set_notification(
                        "No keyword selected".to_string(),
                        crate::types::NotificationType::Warning,
                    );
                }
            }
            AppAction::DoctorRun => {
                self.set_notification(
                    "Doctor checks not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
            AppAction::DoctorTor => {
                self.set_notification(
                    "Tor check not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
            AppAction::DoctorDatabase => {
                self.set_notification(
                    "Database check not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
            AppAction::DoctorNotifications => {
                self.set_notification(
                    "Notification check not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
            AppAction::NotifyTestDiscord => {
                self.set_notification(
                    "Discord test not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
            AppAction::NotifyTestWebhook => {
                self.set_notification(
                    "Webhook test not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
            AppAction::NotifyTestEmail => {
                self.set_notification(
                    "Email test not yet implemented".to_string(),
                    crate::types::NotificationType::Warning,
                );
            }
        }
    }

    fn open_feed_add_form(&mut self) {
        if self.screen != Screen::Feeds {
            self.switch_screen(Screen::Feeds);
        }
        self.feeds_show_form = true;
        self.feeds_form = crate::types::FeedForm::default();
        self.feeds_form_edit_id = None;
        self.input_mode = InputMode::Typing;
        self.form_focus = 0;
        self.set_notification(
            "Add feed form opened".to_string(),
            crate::types::NotificationType::Info,
        );
    }

    fn open_keyword_add_form(&mut self) {
        if self.screen != Screen::Keywords {
            self.switch_screen(Screen::Keywords);
        }
        self.keywords_show_form = true;
        self.keywords_form = crate::types::KeywordForm::default();
        self.keywords_form_edit_id = None;
        self.input_mode = InputMode::Typing;
        self.form_focus = 0;
        self.set_notification(
            "Add keyword form opened".to_string(),
            crate::types::NotificationType::Info,
        );
    }

    /// Returns true if a data-entry form (not a test/assignment overlay) is active.
    /// Used to decide whether global shortcuts like 'q' should be suppressed.
    fn in_form(&self) -> bool {
        match self.screen {
            Screen::Feeds => self.feeds_show_form,
            Screen::Keywords => self.keywords_show_form,
            Screen::Tags => self.tags_show_form,
            Screen::Settings => self.settings_notif_form,
            Screen::Alerts => self.triage_note_input_mode,
            _ => false,
        }
    }

    fn handle_esc(&mut self) {
        if self.filter_active {
            self.clear_current_filter();
            self.filter_active = false;
        } else if self.feeds_detail_view {
            self.feeds_detail_view = false;
        } else if self.feeds_show_form {
            self.feeds_show_form = false;
            self.feeds_form = FeedForm::default();
            self.feeds_form_edit_id = None;
            self.input_mode = InputMode::Normal;
            self.form_focus = 0;
        } else if self.triage_history_view {
            self.triage_history_view = false;
        } else if self.triage_enum_select_mode {
            self.triage_enum_select_mode = false;
            self.triage_enum_target = None;
            self.triage_enum_selected = 0;
        } else if self.triage_note_input_mode {
            self.triage_note_input_mode = false;
            self.triage_note_input.clear();
            self.triage_input_target = crate::types::TriageInputTarget::default();
            self.input_mode = InputMode::Normal;
        } else if self.alerts_detail_view {
            self.alerts_detail_view = false;
        } else if self.alerts_bulk_mode {
            self.alerts_bulk_mode = false;
            self.alerts_selected_bulk.clear();
        } else if self.articles_reader {
            self.articles_reader = false;
            self.refresh_articles();
        } else if self.keywords_show_form {
            self.keywords_show_form = false;
            self.keywords_form = KeywordForm::default();
            self.keywords_form_edit_id = None;
            self.input_mode = InputMode::Normal;
            self.form_focus = 0;
        } else if self.keywords_test_mode {
            self.keywords_test_mode = false;
        } else if self.tags_show_form {
            self.tags_show_form = false;
            self.tags_form = TagForm::default();
            self.tags_form_edit_id = None;
            self.input_mode = InputMode::Normal;
            self.form_focus = 0;
        } else if self.tags_assignment_mode {
            self.tags_assignment_mode = false;
            self.tags_assignment_target = None;
        } else if self.settings_notif_form {
            self.settings_notif_form = false;
            self.settings_notif_form_data = NotificationForm::default();
            self.settings_notif_form_edit_id = None;
            self.input_mode = InputMode::Normal;
            self.form_focus = 0;
        } else {
            self.go_back();
        }
    }

    fn start_filter(&mut self) {
        if !matches!(self.screen, Screen::Dashboard | Screen::Settings) && !self.in_form() {
            self.filter_active = true;
            self.input_mode = InputMode::Typing;
        }
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.clear_current_filter();
                self.filter_active = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.filter_active = false;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.current_filter_mut().pop();
                self.refresh_current_filter_screen();
            }
            KeyCode::Char(c) => {
                self.current_filter_mut().push(c);
                self.refresh_current_filter_screen();
            }
            _ => {}
        }
    }

    fn current_filter_mut(&mut self) -> &mut String {
        match self.screen {
            Screen::Feeds => &mut self.feeds_filter,
            Screen::Alerts => &mut self.alerts_filter,
            Screen::Articles => &mut self.articles_filter,
            Screen::Indicators => &mut self.indicators_filter,
            Screen::EnrichmentQueue => &mut self.enrichment_queue_filter,
            Screen::Keywords => &mut self.keywords_filter,
            Screen::Tags => &mut self.tags_filter,
            Screen::Logs => &mut self.logs_filter,
            _ => &mut self.feeds_filter,
        }
    }

    fn clear_current_filter(&mut self) {
        self.current_filter_mut().clear();
        self.refresh_current_filter_screen();
    }

    fn refresh_current_filter_screen(&mut self) {
        match self.screen {
            Screen::Feeds => self.refresh_feeds(),
            Screen::Alerts => self.refresh_workbench(),
            Screen::Articles => self.refresh_articles(),
            Screen::Indicators => self.refresh_indicators(),
            Screen::EnrichmentQueue => self.refresh_enrichment_queue(),
            Screen::Keywords => self.refresh_keywords(),
            Screen::Tags => self.refresh_tags(),
            Screen::Logs => self.refresh_logs(),
            _ => {}
        }
    }

    fn handle_tag_assignment_key(&mut self, key: KeyEvent) {
        use crate::ui::list::{motion_from_key, move_selection};

        if let Some(motion) = motion_from_key(key, &mut self.pending_g) {
            self.tags_selected = move_selection(self.tags_selected, self.tags_list.len(), motion);
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.tags_assignment_mode = false;
                self.tags_assignment_target = None;
                self.refresh_feeds();
                self.refresh_alerts();
                self.refresh_workbench();
                self.refresh_keywords();
                self.set_notification("Tags updated".into(), NotificationType::Success);
            }
            KeyCode::Char(' ') => {
                if let (Some(target), Some(tag)) = (
                    self.tags_assignment_target.clone(),
                    self.tags_list.get(self.tags_selected),
                ) {
                    let tag_id = tag.id;
                    let assigned = match target {
                        TagAssignmentTarget::Feed(id) => self
                            .db
                            .get_feed_tags(id)
                            .map(|tags| tags.iter().any(|t| t.id == tag_id))
                            .unwrap_or(false),
                        TagAssignmentTarget::Keyword(id) => self
                            .db
                            .get_keyword_tags(id)
                            .map(|tags| tags.iter().any(|t| t.id == tag_id))
                            .unwrap_or(false),
                        TagAssignmentTarget::Alert(id) => self
                            .db
                            .get_alert_tags(id)
                            .map(|tags| tags.iter().any(|t| t.id == tag_id))
                            .unwrap_or(false),
                    };
                    let res = match target {
                        TagAssignmentTarget::Feed(id) if assigned => {
                            self.db.remove_tag_from_feed(id, tag_id)
                        }
                        TagAssignmentTarget::Feed(id) => self.db.assign_tag_to_feed(id, tag_id),
                        TagAssignmentTarget::Keyword(id) if assigned => {
                            self.db.remove_tag_from_keyword(id, tag_id)
                        }
                        TagAssignmentTarget::Keyword(id) => {
                            self.db.assign_tag_to_keyword(id, tag_id)
                        }
                        TagAssignmentTarget::Alert(id) if assigned => {
                            self.db.remove_tag_from_alert(id, tag_id)
                        }
                        TagAssignmentTarget::Alert(id) => self.db.assign_tag_to_alert(id, tag_id),
                    };
                    if res.is_err() {
                        self.set_notification(
                            "Unable to update tag".into(),
                            NotificationType::Error,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_screen_key(&mut self, key: KeyEvent) {
        match self.screen {
            Screen::Dashboard => ui::dashboard::handle_key(self, key),
            Screen::Feeds => ui::feeds::handle_key(self, key),
            Screen::Alerts => ui::alert_workbench::page::handle_key(self, key),
            Screen::Articles => ui::articles::handle_key(self, key),
            Screen::Indicators => ui::indicators::handle_key(self, key),
            Screen::EnrichmentQueue => ui::enrichment_queue::handle_key(self, key),
            Screen::Keywords => ui::keywords::handle_key(self, key),
            Screen::Tags => ui::tags::handle_key(self, key),
            Screen::Logs => ui::logs::handle_key(self, key),
            Screen::Settings => ui::settings::handle_key(self, key),
        }
    }

    pub fn confirm_action(&mut self) {
        if let Some(dialog) = self.show_confirm.take() {
            match dialog {
                ConfirmDialog::DeleteFeed { id, .. } => {
                    let _ = self.db.delete_feed(id);
                    self.refresh_feeds();
                    self.set_notification("Feed deleted".into(), NotificationType::Success);
                }
                ConfirmDialog::DeleteKeyword { id, .. } => {
                    let _ = self.db.delete_keyword(id);
                    self.refresh_keywords();
                    self.refresh_alerts();
                    self.refresh_workbench();
                    self.set_notification("Keyword deleted".into(), NotificationType::Success);
                }
                ConfirmDialog::DeleteTag { id, .. } => {
                    let _ = self.db.delete_tag(id);
                    self.refresh_tags();
                    self.set_notification("Tag deleted".into(), NotificationType::Success);
                }
                ConfirmDialog::DeleteAlert { id } => {
                    let _ = self.db.delete_alert(id);
                    self.refresh_alerts();
                    self.refresh_workbench();
                    self.refresh_indicators();
                    self.refresh_dashboard();
                    self.set_notification("Alert deleted".into(), NotificationType::Success);
                }
                ConfirmDialog::DeleteOldAlerts { cutoff, .. } => {
                    match self.db.delete_old_alerts(cutoff) {
                        Ok(count) => {
                            self.set_notification(
                                format!("Deleted {} old alerts", count),
                                NotificationType::Success,
                            );
                            self.refresh_alerts();
                            self.refresh_workbench();
                            self.refresh_indicators();
                            self.refresh_dashboard();
                        }
                        Err(e) => {
                            self.set_notification(format!("Error: {}", e), NotificationType::Error)
                        }
                    }
                }
                ConfirmDialog::DeleteNotification { id, .. } => {
                    let _ = self.db.delete_notification(id);
                    self.refresh_settings();
                    self.set_notification("Notification deleted".into(), NotificationType::Success);
                }
                ConfirmDialog::BulkDeleteAlerts { .. } => {
                    let ids: Vec<i64> = self.alerts_selected_bulk.iter().copied().collect();
                    match self.db.delete_alerts_by_ids(&ids) {
                        Ok(count) => {
                            self.alerts_bulk_mode = false;
                            self.alerts_selected_bulk.clear();
                            self.refresh_alerts();
                            self.refresh_workbench();
                            self.refresh_indicators();
                            self.refresh_dashboard();
                            self.set_notification(
                                format!("Deleted {} alerts", count),
                                NotificationType::Success,
                            );
                        }
                        Err(e) => {
                            self.set_notification(format!("Error: {}", e), NotificationType::Error)
                        }
                    }
                }
            }
        }
    }

    pub fn refresh_dashboard(&mut self) {
        if let Ok(stats) = self.db.get_stats() {
            self.dashboard_stats = stats;
        }
        let filter = AlertFilter {
            limit: Some(5),
            ..AlertFilter::default()
        };
        if let Ok(alerts) = self.db.list_alerts(&filter) {
            self.dashboard_recent_alerts = alerts;
        }
        if let Ok(dist) = self.db.get_criticality_distribution() {
            self.dashboard_criticality_data = dist;
        }
        if let Ok(counts) = self.db.get_alert_status_counts() {
            self.dashboard_status_counts = counts;
        }
        if let Ok(counts) = self.db.get_alert_disposition_counts() {
            self.dashboard_disposition_counts = counts;
        }
    }

    pub fn refresh_feeds(&mut self) {
        let filter = (!self.feeds_filter.is_empty()).then_some(self.feeds_filter.as_str());
        if let Ok(feeds) = self.db.list_feeds(filter) {
            let mut feeds: Vec<_> = feeds
                .into_iter()
                .map(|f| {
                    let tags = self.db.get_feed_tags(f.id).unwrap_or_default();
                    let status = f.health_status();
                    FeedWithTags {
                        feed: f,
                        tags,
                        status,
                    }
                })
                .collect();
            match self.feeds_sort {
                1 => feeds.sort_by_key(|f| f.feed.name.to_lowercase()),
                2 => feeds.sort_by_key(|f| f.status),
                3 => feeds.sort_by_key(|f| f.feed.last_fetch_at),
                _ => {}
            }
            self.feeds_list = feeds;
        }
        if self.feeds_selected >= self.feeds_list.len() && !self.feeds_list.is_empty() {
            self.feeds_selected = self.feeds_list.len() - 1;
        }
        self.refresh_selected_feed_attempts();
    }

    pub fn refresh_selected_feed_attempts(&mut self) {
        self.feeds_selected_attempts.clear();
        if let Some(feed) = self.feeds_list.get(self.feeds_selected) {
            self.feeds_selected_attempts = self
                .db
                .list_feed_fetch_attempts(feed.feed.id, 5)
                .unwrap_or_default();
        }
    }

    pub fn refresh_alerts(&mut self) {
        let filter = AlertFilter {
            text: if self.alerts_filter.is_empty() {
                None
            } else {
                Some(self.alerts_filter.clone())
            },
            criticality: self.alerts_filter_criticality,
            unread_only: self.alerts_filter_unread_only,
            status: self.alerts_filter_status,
            disposition: self.alerts_filter_disposition,
            open_only: self.alerts_hide_closed,
            limit: Some(500),
            ..AlertFilter::default()
        };
        if let Ok(alerts) = self.db.list_alerts(&filter) {
            self.alerts_list = alerts;
        }
        if self.alerts_selected >= self.alerts_list.len() && !self.alerts_list.is_empty() {
            self.alerts_selected = self.alerts_list.len() - 1;
        }
    }

    // ── Alert workbench (split-pane view) service delegators ─────────────────
    //
    // Thin wrappers over the free-function app services so the TUI can load
    // workbench data through `App` without reaching into `Db` directly.

    /// Load the alert list as workbench view models for the given filter.
    pub fn load_alert_workbench_items(
        &self,
        filter: &AlertFilterState,
    ) -> Result<Vec<AlertListItem>> {
        load_alert_workbench_list(&self.db, filter)
    }

    /// Load the full selected-alert bundle (details + context data).
    pub fn load_selected_alert_bundle(
        &self,
        alert_id: i64,
    ) -> Result<Option<AlertWorkbenchBundle>> {
        load_alert_workbench_bundle(&self.db, alert_id)
    }

    // ── Alert workbench lifecycle ─────────────────────────────────────────────
    //
    // `refresh_workbench` reloads the list (preserving the selected alert id
    // where possible) and the selected alert's context bundle. It syncs the
    // workbench filter from the global `/`-filter fields so the existing filter
    // bar drives the workbench.

    /// Reload the alert list + selected-alert bundle for the workbench.
    ///
    /// Any load failure is recorded in `workbench.last_error` (stale data is
    /// kept) and surfaced in the status bar / panes. A successful full reload
    /// clears the error.
    pub fn refresh_workbench(&mut self) {
        self.workbench.alert_filter = AlertFilterState {
            text: self.alerts_filter.clone(),
            severity: self.alerts_filter_criticality,
            status: self.alerts_filter_status,
            disposition: self.alerts_filter_disposition,
            unread_only: self.alerts_filter_unread_only,
            hide_closed: self.alerts_hide_closed,
        };
        // A fresh full reload starts with a clean slate.
        self.workbench.set_error(None);

        match self.load_alert_workbench_items(&self.workbench.alert_filter) {
            Ok(items) => {
                self.workbench
                    .restore_selection_by_id(items.len(), |i| items.get(i).map(|x| x.id));
                self.workbench_items = items;
            }
            Err(e) => self
                .workbench
                .set_error(Some(format!("Failed to load alerts: {e}"))),
        }
        self.workbench_sync_selection();

        // Load the bundle WITHOUT clearing the error on success — a list error
        // recorded above must stay visible. Only a bundle failure adds one.
        self.workbench_bundle = match self.workbench.selected_alert_id() {
            Some(id) => match self.load_selected_alert_bundle(id) {
                Ok(b) => b,
                Err(e) => {
                    self.workbench
                        .set_error(Some(format!("Failed to load alert context: {e}")));
                    None
                }
            },
            None => None,
        };
    }

    /// Reload the bundle for the currently selected alert (no list reload).
    /// Used after a selection move: a successful load clears any prior error.
    pub fn refresh_workbench_bundle(&mut self) {
        self.workbench_bundle = match self.workbench.selected_alert_id() {
            Some(id) => match self.load_selected_alert_bundle(id) {
                Ok(b) => {
                    self.workbench.set_error(None);
                    b
                }
                Err(e) => {
                    self.workbench
                        .set_error(Some(format!("Failed to load alert context: {e}")));
                    None
                }
            },
            None => None,
        };
    }

    /// Call after a selection move: sync the selected id from the (clamped)
    /// index, reset detail/context scroll, and reload the bundle.
    pub fn workbench_reload_selected(&mut self) {
        self.workbench_sync_selection();
        self.workbench.reset_scroll_for_selection_change();
        self.refresh_workbench_bundle();
    }

    /// Move the workbench alert-list selection by `motion`, reconcile the
    /// selected alert id from the loaded items, reset detail/context scroll,
    /// and reload the bundle. Single entry point for `j/k`/`gg`/`G`/half-page
    /// navigation — callers must never write the (private) selection fields.
    pub fn workbench_move_selection(&mut self, motion: crate::ui::list::ListMotion) {
        let len = self.workbench_items.len();
        let desired =
            crate::ui::list::move_selection(self.workbench.selected_alert_index(), len, motion);
        let items = &self.workbench_items;
        self.workbench
            .set_selection(desired, len, |i| items.get(i).map(|x| x.id));
        self.workbench.reset_scroll_for_selection_change();
        self.refresh_workbench_bundle();
    }

    /// Clamp the selection index to the list and derive `selected_alert_id`.
    fn workbench_sync_selection(&mut self) {
        let len = self.workbench_items.len();
        let desired = self.workbench.selected_alert_index();
        let items = &self.workbench_items;
        self.workbench
            .set_selection(desired, len, |i| items.get(i).map(|x| x.id));
    }

    pub fn refresh_articles(&mut self) {
        let filter = FeedItemFilter {
            text: if self.articles_filter.is_empty() {
                None
            } else {
                Some(self.articles_filter.clone())
            },
            unread_only: self.articles_unread_only,
            limit: Some(500),
            ..FeedItemFilter::default()
        };
        if let Ok(items) = self.db.list_feed_items(&filter) {
            self.articles_list = items;
        }
        if self.articles_selected >= self.articles_list.len() && !self.articles_list.is_empty() {
            self.articles_selected = self.articles_list.len() - 1;
        }
    }

    pub fn refresh_indicators(&mut self) {
        let search = IndicatorSearch {
            text: if self.indicators_filter.is_empty() {
                None
            } else {
                Some(self.indicators_filter.clone())
            },
            indicator_type: self.indicators_filter_type,
            limit: Some(500),
        };
        if let Ok(indicators) = self.db.search_indicators(&search) {
            self.indicators_list = indicators;
        }
        if self.indicators_selected >= self.indicators_list.len()
            && !self.indicators_list.is_empty()
        {
            self.indicators_selected = self.indicators_list.len() - 1;
        }
    }

    pub fn fetch_selected_feed(&mut self) {
        let Some(feed) = self
            .feeds_list
            .get(self.feeds_selected)
            .map(|entry| entry.feed.clone())
        else {
            return;
        };
        let template = feed
            .api_template_id
            .and_then(|id| self.db.get_template(id).ok().flatten());

        let outcome = crate::feed::FeedManager::run_fetch_attempt(
            &feed,
            template,
            self.config.network.tls_trust_store,
        );
        match outcome.result {
            Some(result) => {
                let item_count = result.items.len();
                let keywords = self.db.list_keywords(true).unwrap_or_default();
                match crate::alert::AlertEngine::process_feed_result_with_config(
                    &self.db,
                    &feed,
                    &result,
                    &keywords,
                    &self.config,
                ) {
                    Ok(alerts) => {
                        let _ = self.db.record_feed_fetch_outcome(
                            feed.id,
                            &outcome.attempt,
                            Some(result.content_hash.as_str()),
                        );
                        let _ = self.db.add_health_log(feed.id, FeedStatus::Healthy, None);
                        let _ = self
                            .db
                            .prune_health_logs(feed.id, self.config.max_health_log_entries);
                        self.refresh_feeds();
                        self.refresh_articles();
                        self.refresh_alerts();
                        self.refresh_workbench();
                        self.refresh_indicators();
                        self.refresh_dashboard();
                        self.refresh_logs();
                        self.set_notification(
                            format!(
                                "Fetched {} items and created {} alerts",
                                item_count,
                                alerts.len()
                            ),
                            NotificationType::Success,
                        );
                    }
                    Err(e) => {
                        let mut attempt = outcome.attempt.clone();
                        attempt.success = false;
                        attempt.diagnostic = Some(crate::feed::diagnostics::FetchDiagnostic {
                            phase: crate::feed::diagnostics::FetchFailurePhase::DatabaseWrite,
                            kind: crate::feed::diagnostics::FetchFailureKind::DatabaseError,
                            summary: "Feed result could not be stored".to_string(),
                            detail: Some(e.to_string()),
                            http_status: attempt.http_status,
                            url: attempt.url.clone(),
                            final_url: attempt.final_url.clone(),
                            elapsed_ms: attempt.elapsed_ms,
                        });
                        let _ = self.db.record_feed_fetch_outcome(feed.id, &attempt, None);
                        self.set_notification(
                            format!("Feed processed but storing alerts/items failed: {}", e),
                            NotificationType::Error,
                        );
                    }
                }
            }
            None => {
                let message = outcome
                    .attempt
                    .diagnostic
                    .as_ref()
                    .map(|diagnostic| diagnostic.summary.as_str())
                    .unwrap_or("Fetch failed");
                let _ = self
                    .db
                    .record_feed_fetch_outcome(feed.id, &outcome.attempt, None);
                let _ = self
                    .db
                    .add_health_log(feed.id, FeedStatus::Error, Some(message));
                let _ = self
                    .db
                    .prune_health_logs(feed.id, self.config.max_health_log_entries);
                self.refresh_feeds();
                self.refresh_logs();
                self.set_notification(
                    format!("Fetch failed: {}", message),
                    NotificationType::Error,
                );
            }
        }
    }

    pub fn refresh_keywords(&mut self) {
        if let Ok(kws) = self.db.list_keywords(false) {
            self.keyword_tags.clear();
            let query = self.keywords_filter.to_lowercase();
            self.keywords_list = kws
                .into_iter()
                .filter(|k| {
                    query.is_empty()
                        || k.pattern.to_lowercase().contains(&query)
                        || format!("{:?}", k.criticality)
                            .to_lowercase()
                            .contains(&query)
                })
                .inspect(|k| {
                    let tags = self.db.get_keyword_tags(k.id).unwrap_or_default();
                    self.keyword_tags.insert(k.id, tags);
                })
                .collect();
        }
        if self.keywords_selected >= self.keywords_list.len() && !self.keywords_list.is_empty() {
            self.keywords_selected = self.keywords_list.len() - 1;
        }
    }

    pub fn refresh_tags(&mut self) {
        if let Ok(tags) = self.db.list_tags() {
            let query = self.tags_filter.to_lowercase();
            self.tags_list = tags
                .into_iter()
                .filter(|t| {
                    query.is_empty()
                        || t.name.to_lowercase().contains(&query)
                        || t.description
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                })
                .collect();
            self.tag_usage_counts = self.db.get_tag_usage_counts().unwrap_or_default();
        }
        if self.tags_selected >= self.tags_list.len() && !self.tags_list.is_empty() {
            self.tags_selected = self.tags_list.len() - 1;
        }
    }

    pub fn refresh_logs(&mut self) {
        if let Ok(logs) = self.db.get_health_logs(self.logs_filter_feed, 500) {
            let query = self.logs_filter.to_lowercase();
            self.logs_list = logs
                .into_iter()
                .filter(|log| {
                    query.is_empty()
                        || log.status.label().to_lowercase().contains(&query)
                        || log
                            .error_message
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                        || self
                            .feeds_list
                            .iter()
                            .find(|ft| ft.feed.id == log.feed_id)
                            .map(|ft| ft.feed.name.to_lowercase().contains(&query))
                            .unwrap_or(false)
                })
                .collect();
        }
        if self.logs_selected >= self.logs_list.len() && !self.logs_list.is_empty() {
            self.logs_selected = self.logs_list.len() - 1;
        }
    }

    pub fn refresh_enrichment_queue(&mut self) {
        if let Ok(jobs) = self.db.list_enrichment_jobs(500) {
            let query = self.enrichment_queue_filter.to_lowercase();
            self.enrichment_queue_list = jobs
                .into_iter()
                .filter(|job| {
                    query.is_empty()
                        || job.provider_name.to_lowercase().contains(&query)
                        || job.provider_type.to_lowercase().contains(&query)
                        || job.status.to_lowercase().contains(&query)
                        || job.indicator_value.to_lowercase().contains(&query)
                        || format!("{:?}", job.indicator_type)
                            .to_lowercase()
                            .contains(&query)
                        || job
                            .error_message
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                })
                .collect();
        }
        if self.enrichment_queue_selected >= self.enrichment_queue_list.len()
            && !self.enrichment_queue_list.is_empty()
        {
            self.enrichment_queue_selected = self.enrichment_queue_list.len() - 1;
        }
    }

    pub fn refresh_settings(&mut self) {
        if let Ok(notifs) = self.db.list_notifications() {
            self.settings_notifications = notifs;
        }
        if let Ok(providers) = self.db.list_enrichment_providers() {
            self.settings_enrichment_providers = providers;
        }
        if self.settings_enrichment_provider_selected >= self.settings_enrichment_providers.len()
            && !self.settings_enrichment_providers.is_empty()
        {
            self.settings_enrichment_provider_selected =
                self.settings_enrichment_providers.len() - 1;
        }
        self.settings_retention_days = self.config.alert_retention_days;
        self.settings_theme_name = self.config.theme.clone();
        self.settings_tls_trust_store = self.config.network.tls_trust_store;
    }

    pub fn start_auto_fetch(&mut self) {
        if self.auto_fetcher.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let fetcher = match AutoFetcher::spawn(
            self.paths.db_file.clone(),
            self.settings_auto_fetch_interval,
            self.settings_tls_trust_store,
            tx,
        ) {
            Ok(fetcher) => fetcher,
            Err(e) => {
                eprintln!("Failed to start auto-fetcher: {}", e);
                return;
            }
        };
        self.auto_fetcher = Some(fetcher);
        self.auto_fetch_rx = Some(rx);
    }

    pub fn stop_auto_fetch(&mut self) {
        if let Some(fetcher) = self.auto_fetcher.take() {
            fetcher.stop();
        }
        self.auto_fetch_rx = None;
    }

    pub fn restart_auto_fetch(&mut self) {
        self.stop_auto_fetch();
        if self.settings_auto_fetch_enabled {
            self.start_auto_fetch();
        }
    }
}

#[cfg(test)]
mod workbench_tests {
    //! Integration tests for the alert workbench app services (Ticket 02).
    //!
    //! These exercise the full storage → view-model path: they seed a temp DB
    //! and assert the bundle/list loaders return the right view-model shapes
    //! (never raw DB rows). Missing optional data yields empty lists, not errors.
    use super::*;
    use crate::db::{AlertCreate, Db, EnrichmentProviderCreate, FeedCreate, KeywordCreate};
    use crate::types::{Criticality, FeedType};
    use sentinel_enrichment::{EnrichmentResult, Reputation};
    use sentinel_ioc::{ExtractedIndicator, IndicatorType};
    use std::path::PathBuf;

    fn temp_db(name: &str) -> (Db, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "threatdeck-workbench-{}-{}.db",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
        db.init_schema().unwrap();
        (db, path)
    }

    fn seed_feed_keyword(db: &Db) -> (i64, i64) {
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "IOC Feed".into(),
                url: "https://ioc.example.test/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "ransomware".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();
        (feed_id, keyword_id)
    }

    fn create_alert(
        db: &Db,
        feed_id: i64,
        keyword_id: i64,
        title: &str,
        metadata: Option<&str>,
    ) -> i64 {
        db.create_alert(&AlertCreate {
            feed_id,
            keyword_id,
            title: Some(title.into()),
            content_snippet: "Ransomware mentions CVE-2025-12345".into(),
            criticality: Criticality::High,
            content_hash: format!("hash-{title}"),
            metadata_json: metadata.map(str::to_string),
        })
        .unwrap()
    }

    #[test]
    fn bundle_loads_alert_details_and_metadata() {
        let (db, path) = temp_db("details");
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        let alert_id = create_alert(
            &db,
            feed_id,
            keyword_id,
            "Ransomware alert",
            Some(r#"{"source":"feed"}"#),
        );

        let bundle = load_alert_workbench_bundle(&db, alert_id).unwrap().unwrap();

        let detail = bundle.detail.expect("detail should be present");
        assert_eq!(detail.id, alert_id);
        assert_eq!(detail.title.as_deref(), Some("Ransomware alert"));
        assert_eq!(detail.severity, Criticality::High);
        assert_eq!(detail.feed_name, "IOC Feed");
        assert_eq!(
            detail.feed_url.as_deref(),
            Some("https://ioc.example.test/feed.xml")
        );
        assert_eq!(detail.keyword_pattern, "ransomware");
        assert_eq!(detail.status, crate::types::AlertStatus::New);
        assert_eq!(
            bundle.metadata_json.as_deref(),
            Some(r#"{"source":"feed"}"#)
        );
        // No indicators/history seeded yet.
        assert!(bundle.indicators.is_empty());
        assert!(bundle.triage_history.is_empty());

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bundle_with_no_indicators_returns_empty_list() {
        let (db, path) = temp_db("no-iocs");
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        let alert_id = create_alert(&db, feed_id, keyword_id, "Plain alert", None);

        let bundle = load_alert_workbench_bundle(&db, alert_id).unwrap().unwrap();
        assert!(bundle.indicators.is_empty());

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bundle_loads_indicators_with_enrichment() {
        let (db, path) = temp_db("iocs");
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        let alert_id = create_alert(&db, feed_id, keyword_id, "IOC alert", None);

        // Store + link an indicator to the alert.
        let ids = db
            .store_extracted_indicators(
                &[ExtractedIndicator {
                    indicator_type: IndicatorType::Cve,
                    value: "CVE-2025-12345".into(),
                    normalized_value: "CVE-2025-12345".into(),
                    source_field: "content_snippet".into(),
                    start_offset: 0,
                    end_offset: 14,
                    surrounding_text: "CVE-2025-12345".into(),
                    confidence_hint: Some(90),
                }],
                Some(alert_id),
                None,
                Some(feed_id),
            )
            .unwrap();

        // Add an enrichment result for that indicator.
        let provider_id = db
            .create_enrichment_provider(&EnrichmentProviderCreate {
                name: "cisa-kev".into(),
                provider_type: "cisa_kev".into(),
                enabled: true,
                supports_types: vec![IndicatorType::Cve],
                ..EnrichmentProviderCreate::default()
            })
            .unwrap();
        db.store_enrichment_result(
            ids[0],
            provider_id,
            &EnrichmentResult {
                provider_name: "cisa-kev".into(),
                indicator_type: IndicatorType::Cve,
                normalized_value: "CVE-2025-12345".into(),
                reputation: Reputation::Malicious,
                score: Some(95),
                verdict: Some("known-exploited".into()),
                summary: Some("Listed in CISA KEV".into()),
                raw_json: serde_json::json!({"kev": true}),
                expires_at: None,
            },
        )
        .unwrap();

        let bundle = load_alert_workbench_bundle(&db, alert_id).unwrap().unwrap();
        assert_eq!(bundle.indicators.len(), 1);
        let indicator = &bundle.indicators[0];
        assert_eq!(indicator.indicator_type, IndicatorType::Cve);
        assert_eq!(indicator.normalized_value, "CVE-2025-12345");
        assert_eq!(indicator.type_label(), "CVE");
        assert_eq!(indicator.enrichment.len(), 1);
        let enrich = &indicator.enrichment[0];
        assert_eq!(enrich.reputation.as_deref(), Some("Malicious"));
        assert_eq!(enrich.score, Some(95));
        assert_eq!(enrich.verdict.as_deref(), Some("known-exploited"));

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bundle_loads_triage_history() {
        let (db, path) = temp_db("history");
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        let alert_id = create_alert(&db, feed_id, keyword_id, "History alert", None);

        // A status change records a triage event.
        db.update_alert_status(
            alert_id,
            crate::types::AlertStatus::Acknowledged,
            Some("acknowledged by analyst"),
        )
        .unwrap();

        let bundle = load_alert_workbench_bundle(&db, alert_id).unwrap().unwrap();
        assert_eq!(bundle.triage_history.len(), 1);
        let event = &bundle.triage_history[0];
        assert_eq!(event.event_type, "status_changed");
        assert_eq!(event.new_value.as_deref(), Some("Acknowledged"));
        assert_eq!(event.note.as_deref(), Some("acknowledged by analyst"));
        // Detail should reflect the new status.
        assert_eq!(
            bundle.detail.as_ref().unwrap().status,
            crate::types::AlertStatus::Acknowledged
        );

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bundle_for_missing_alert_returns_none() {
        let (db, path) = temp_db("missing");
        assert!(load_alert_workbench_bundle(&db, 999_999).unwrap().is_none());

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn list_loader_returns_view_models() {
        let (db, path) = temp_db("list");
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        // `init_schema` seeds a demo catalog, so compare against a baseline.
        let before = load_alert_workbench_list(&db, &AlertFilterState::default())
            .unwrap()
            .len();
        let id1 = create_alert(&db, feed_id, keyword_id, "First alert", None);
        let id2 = create_alert(&db, feed_id, keyword_id, "Second alert", None);

        let items = load_alert_workbench_list(&db, &AlertFilterState::default()).unwrap();
        assert_eq!(items.len(), before + 2);
        let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        // The newly created items carry feed/keyword context as view models.
        for item in items.iter().filter(|i| i.id == id1 || i.id == id2) {
            assert_eq!(item.feed_name, "IOC Feed");
            assert_eq!(item.keyword_pattern, "ransomware");
            assert_eq!(item.severity, Criticality::High);
        }

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn list_loader_honours_hide_closed_filter() {
        let (db, path) = temp_db("filter");
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        let open_id = create_alert(&db, feed_id, keyword_id, "Open alert", None);
        let closed_id = create_alert(&db, feed_id, keyword_id, "Closed alert", None);
        // Close one (requires a disposition per Alert.can_close()).
        db.update_alert_disposition(
            closed_id,
            crate::types::AlertDisposition::FalsePositive,
            None,
        )
        .unwrap();
        db.close_alert(
            closed_id,
            crate::types::AlertDisposition::FalsePositive,
            None,
        )
        .unwrap();

        let filter = AlertFilterState {
            hide_closed: true,
            ..AlertFilterState::default()
        };
        let items = load_alert_workbench_list(&db, &filter).unwrap();
        let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        assert!(ids.contains(&open_id));
        assert!(!ids.contains(&closed_id));

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    // ── Loading / error states (Ticket 08) ─────────────────────────────────

    fn build_app(db: Db) -> App {
        use crate::types::Screen;
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: PathBuf::new(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = Screen::Alerts;
        app
    }

    #[test]
    fn refresh_workbench_records_error_when_schema_missing() {
        // A DB opened WITHOUT init_schema has no `alerts` table, so the list
        // load deterministically fails — exercising the error-capture path.
        let path = std::env::temp_dir().join(format!(
            "threatdeck-wb-err-{}-{}.db",
            std::process::id(),
            "noschema"
        ));
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap(); // intentionally no init_schema
        let mut app = build_app(db);
        app.refresh_workbench();

        let err = app
            .workbench
            .last_error
            .as_deref()
            .expect("a load error should be recorded");
        assert!(
            err.contains("Failed to load alerts"),
            "unexpected error text: {err}"
        );
        // Stale (empty) state is kept; no panic; no selection.
        assert!(app.workbench_items.is_empty());
        assert!(app.workbench.selected_alert_id().is_none());

        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn successful_refresh_clears_workbench_error() {
        let (db, path) = temp_db("clearerr");
        let mut app = build_app(db);
        // Plant a stale error; a successful reload must clear it.
        app.workbench.set_error(Some("stale".into()));
        app.refresh_workbench();
        assert!(
            app.workbench.last_error.is_none(),
            "error should clear after a successful reload"
        );
        // init_schema seeded a demo catalog, so the list is non-empty.
        assert!(!app.workbench_items.is_empty());

        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    // ── Command palette dispatch (Ticket 10) ────────────────────────────────

    fn build_app_with_alert(name: &str) -> (App, PathBuf, i64) {
        let (db, path) = temp_db(name);
        let (feed_id, keyword_id) = seed_feed_keyword(&db);
        let id = db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("PaletteAlert".into()),
                content_snippet: "s".into(),
                content_hash: format!("palette-{name}"),
                criticality: Criticality::High,
                metadata_json: None,
            })
            .unwrap();
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: std::env::temp_dir(),
            config_file: PathBuf::new(),
            db_file: path.clone(),
        };
        let mut app = App::new(db, AppConfig::default(), paths);
        app.screen = Screen::Alerts;
        app.alerts_filter = "PaletteAlert".to_string();
        app.alerts_hide_closed = false;
        app.refresh_workbench();
        (app, path, id)
    }

    #[test]
    fn palette_focus_sets_pane_and_switches_screen() {
        let (mut app, path, _id) = build_app_with_alert("focus");
        app.screen = Screen::Dashboard; // start on another screen
        app.handle_app_action(AppAction::AlertFocusDetails);
        assert_eq!(app.screen, Screen::Alerts);
        assert_eq!(app.workbench.focused_pane, AlertPane::AlertDetails);
        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn palette_tab_command_switches_context_tab() {
        let (mut app, path, _id) = build_app_with_alert("tab");
        app.handle_app_action(AppAction::AlertTabMetadata);
        assert_eq!(app.workbench.bottom_tab, AlertContextTab::Metadata);
        app.handle_app_action(AppAction::AlertTabHistory);
        assert_eq!(app.workbench.bottom_tab, AlertContextTab::TriageHistory);
        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn palette_acknowledge_triages_selected_alert() {
        let (mut app, path, _id) = build_app_with_alert("ack");
        app.handle_app_action(AppAction::AlertAcknowledge);
        let status = app
            .workbench_bundle
            .as_ref()
            .and_then(|b| b.detail.as_ref())
            .unwrap()
            .status;
        assert_eq!(status, crate::types::AlertStatus::Acknowledged);
        assert_eq!(
            app.notification.as_ref().unwrap().1,
            crate::types::NotificationType::Success
        );
        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn palette_mark_read_uses_workbench_selection() {
        let (mut app, path, id) = build_app_with_alert("markread");
        app.handle_app_action(AppAction::AlertMarkSelectedRead);
        let item = app
            .workbench_items
            .iter()
            .find(|i| i.id == id)
            .expect("selected alert still in list");
        assert!(item.read, "selected alert should be marked read");
        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn palette_export_visible_writes_file() {
        let (mut app, path, _id) = build_app_with_alert("exportvis");
        app.handle_app_action(AppAction::AlertExportVisibleMarkdown);
        let notif = app.notification.as_ref().expect("notification set");
        assert_eq!(notif.1, crate::types::NotificationType::Success);
        // "Exported N alerts: <path>" — extract the path and verify the file.
        let msg = &notif.0;
        let exported = msg.split(':').nth(1).unwrap().trim();
        assert!(
            std::path::Path::new(exported).exists(),
            "visible-export file missing: {exported}"
        );
        drop(app);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn palette_triage_without_selection_warns() {
        let (mut app, path, _id) = build_app_with_alert("notri");
        app.alerts_filter = "zzzz-none".into();
        app.refresh_workbench();
        assert!(app.workbench.selected_alert_id().is_none());
        app.handle_app_action(AppAction::AlertAcknowledge);
        assert_eq!(
            app.notification.as_ref().unwrap().1,
            crate::types::NotificationType::Warning
        );
        drop(app);
        let _ = std::fs::remove_file(&path);
    }
}
