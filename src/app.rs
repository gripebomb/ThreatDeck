use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::auto_fetch::{AutoFetchMessage, AutoFetcher};
use crate::config::{AppConfig, Paths, TlsTrustStore};
use crate::db::{
    AlertFilter, Db, EnrichmentJobWithContext, EnrichmentProviderRecord, IndicatorRecord,
    IndicatorSearch,
};
use crate::theme::{get_runtime_theme, Theme};
use crate::types::*;
use crate::ui;
use crate::ui::alerts::state::{AlertContextTab, AlertPane, AlertWorkbenchState};
use crate::ui::command_palette::{AppAction, CommandAction, CommandId, CommandPalette, ModalKind};
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Typing,
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

    // Alert Workbench
    pub workbench_state: AlertWorkbenchState,
    pub workbench_bundle: Option<AlertWorkbenchBundle>,
    pub triage_input_target: crate::types::TriageInputTarget,

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
            workbench_state: AlertWorkbenchState::default(),
            workbench_bundle: None,
            triage_input_target: crate::types::TriageInputTarget::default(),
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
            Screen::Alerts => self.refresh_alerts(),
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
                let context = self.command_context();
                self.command_palette.state.open_colon(&context);
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let context = self.command_context();
                self.command_palette.state.open_fuzzy(&context);
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

    fn command_context(&self) -> ui::command_palette::registry::CommandContext {
        let has_channel = |channel| {
            self.settings_notifications
                .iter()
                .any(|notification| notification.enabled && notification.channel == channel)
        };
        ui::command_palette::registry::CommandContext {
            current_screen: self.screen,
            has_selected_feed: self.feeds_list.get(self.feeds_selected).is_some(),
            has_selected_alert: self.workbench_state.selected_alert_id().is_some(),
            has_selected_keyword: self.keywords_list.get(self.keywords_selected).is_some(),
            discord_configured: has_channel(NotificationChannel::Discord),
            webhook_configured: has_channel(NotificationChannel::Webhook),
            email_configured: has_channel(NotificationChannel::Email),
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

    fn handle_app_action(&mut self, action: AppAction) {
        match action {
            AppAction::Refresh => {
                match self.screen {
                    Screen::Dashboard => self.refresh_dashboard(),
                    Screen::Feeds => self.refresh_feeds(),
                    Screen::Alerts => self.refresh_alerts(),
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
                self.refresh_alerts();
                self.switch_screen(Screen::Alerts);
                self.set_notification(
                    "Showing unread alerts".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
            AppAction::AlertShowCritical => {
                self.alerts_filter_criticality = Some(crate::types::Criticality::Critical);
                self.refresh_alerts();
                self.switch_screen(Screen::Alerts);
                self.set_notification(
                    "Showing critical alerts".to_string(),
                    crate::types::NotificationType::Info,
                );
            }
            AppAction::AlertMarkSelectedRead => {
                if let Some(a) = self.alerts_list.get(self.alerts_selected) {
                    let _ = self.db.mark_alert_read(a.alert.id, true);
                    self.refresh_alerts();
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
                if let Some(a) = self.alerts_list.get(self.alerts_selected) {
                    let _ = self.db.mark_alert_read(a.alert.id, false);
                    self.refresh_alerts();
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
                self.refresh_alerts();
                self.refresh_dashboard();
                self.set_notification(
                    "All alerts marked as read".to_string(),
                    crate::types::NotificationType::Success,
                );
            }
            AppAction::AlertExportSelectedMarkdown => {
                self.export_selected_alert_markdown();
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
                match report_service.export_visible_alerts_report(
                    &self.db,
                    &self.alerts_list,
                    &filter,
                    &options,
                    &export_dir,
                ) {
                    Ok(result) => {
                        self.set_notification(
                            format!(
                                "Exported {} alerts: {}",
                                self.alerts_list.len(),
                                result.path.display()
                            ),
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
            AppAction::AlertFocusList => self.focus_alert_pane(AlertPane::List),
            AppAction::AlertFocusDetails => self.focus_alert_pane(AlertPane::Details),
            AppAction::AlertFocusContext => self.focus_alert_pane(AlertPane::Context),
            AppAction::AlertContextIocs => {
                self.set_alert_context_tab(AlertContextTab::Iocs);
            }
            AppAction::AlertContextMetadata => {
                self.set_alert_context_tab(AlertContextTab::Metadata);
            }
            AppAction::AlertContextHistory => {
                self.set_alert_context_tab(AlertContextTab::TriageHistory);
            }
            AppAction::AlertTriageAcknowledge => self.triage_acknowledge(),
            AppAction::AlertTriageInvestigate => self.triage_investigate(),
            AppAction::AlertTriageEscalate => self.triage_escalate(),
            AppAction::AlertTriageClose => self.triage_close(),
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
            Screen::Alerts => self.refresh_alerts(),
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
            Screen::Alerts => ui::alerts::handle_key(self, key),
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
        self.refresh_workbench();
    }

    pub fn refresh_workbench(&mut self) {
        let prev_id = self.workbench_state.selected_alert_id();
        let items: Vec<AlertListItem> = self.alerts_list.iter().map(AlertListItem::from).collect();
        self.workbench_state.set_alerts(items);
        // Preserve selection by alert ID when possible
        if let Some(id) = prev_id {
            if let Some(idx) = self.workbench_state.alerts.iter().position(|a| a.id == id) {
                self.workbench_state.selected_alert_index = idx;
            }
        }
        self.alerts_selected = self.workbench_state.selected_alert_index;
        self.workbench_bundle = self
            .workbench_state
            .selected_alert_id()
            .and_then(|id| self.get_alert_workbench_bundle(id));
    }

    /// Reload only the workbench bundle for the current selection,
    /// without re-syncing the alert list. Called after selection changes.
    pub fn refresh_workbench_bundle(&mut self) {
        self.alerts_selected = self.workbench_state.selected_alert_index;
        self.workbench_bundle = self
            .workbench_state
            .selected_alert_id()
            .and_then(|id| self.get_alert_workbench_bundle(id));
    }

    /// Export the selected alert as a Markdown report.
    pub fn export_selected_alert_markdown(&mut self) {
        let Some(alert) = self.alerts_list.get(self.alerts_selected) else {
            self.set_notification(
                "No alert selected".to_string(),
                crate::types::NotificationType::Warning,
            );
            return;
        };
        let alert_id = alert.alert.id;
        let report_service = crate::report::ReportService::new();
        let options = threatdeck_report::ReportExportOptions {
            report_type: threatdeck_report::ReportType::Alert,
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
        match report_service.export_alert_report(&self.db, alert_id, &options, &export_dir) {
            Ok(result) => {
                self.set_notification(
                    format!("Exported: {}", result.path.display()),
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

    /// Queue enrichment for a single indicator. Checks the enrichment config.
    pub fn enrich_indicator(&mut self, indicator_id: i64, label: &str) {
        if !self.config.enrichment.enabled {
            self.set_notification(
                "Enrichment is disabled in settings".to_string(),
                crate::types::NotificationType::Warning,
            );
            return;
        }
        match self
            .db
            .queue_enrichment_jobs_for_indicators(&[indicator_id])
        {
            Ok(job_ids) if job_ids.is_empty() => {
                self.set_notification(
                    format!("No enrichment jobs queued for {}", label),
                    crate::types::NotificationType::Info,
                );
            }
            Ok(job_ids) => {
                self.refresh_enrichment_queue();
                self.set_notification(
                    format!("Queued {} enrichment job(s) for {}", job_ids.len(), label),
                    crate::types::NotificationType::Success,
                );
            }
            Err(e) => {
                self.set_notification(
                    format!("Unable to queue enrichment: {}", e),
                    crate::types::NotificationType::Error,
                );
            }
        }
    }

    // ── Shared triage actions ───────────────────────────────────────────────
    // Called from both keyboard handler (events.rs) and command palette.

    /// Acknowledge the selected alert.
    pub fn triage_acknowledge(&mut self) {
        self.triage_set_status(
            AlertStatus::Acknowledged,
            "Alert acknowledged",
            NotificationType::Success,
        );
    }

    /// Start investigating the selected alert.
    pub fn triage_investigate(&mut self) {
        self.triage_set_status(
            AlertStatus::Investigating,
            "Investigation started",
            NotificationType::Success,
        );
    }

    /// Escalate the selected alert.
    pub fn triage_escalate(&mut self) {
        self.triage_set_status(
            AlertStatus::Escalated,
            "Alert escalated",
            NotificationType::Warning,
        );
    }

    /// Close the selected alert (requires a disposition).
    pub fn triage_close(&mut self) {
        self.alerts_selected = self.workbench_state.selected_alert_index;
        let Some(a) = self.alerts_list.get(self.alerts_selected) else {
            self.set_notification("No alert selected".into(), NotificationType::Warning);
            return;
        };
        if !a.alert.can_close() {
            self.set_notification(
                "Select disposition before closing (T)".into(),
                NotificationType::Warning,
            );
            return;
        }
        let result = self
            .db
            .close_alert(a.alert.id, a.alert.disposition, None)
            .and_then(|()| self.db.mark_alert_read(a.alert.id, true));
        self.finish_triage_mutation(result, "Alert closed".into(), NotificationType::Success);
    }

    /// Reopen the selected alert.
    pub fn triage_reopen(&mut self) {
        self.alerts_selected = self.workbench_state.selected_alert_index;
        let Some(a) = self.alerts_list.get(self.alerts_selected) else {
            self.set_notification("No alert selected".into(), NotificationType::Warning);
            return;
        };
        if a.alert.status != AlertStatus::Closed {
            self.set_notification("Alert is not closed".into(), NotificationType::Warning);
            return;
        }
        let result = self.db.reopen_alert(a.alert.id, None);
        self.finish_triage_mutation(result, "Alert reopened".into(), NotificationType::Success);
    }

    /// Focus a specific alert workbench pane. Navigates to Alerts first if needed.
    pub fn focus_alert_pane(&mut self, pane: AlertPane) {
        if self.screen != Screen::Alerts {
            self.switch_screen(Screen::Alerts);
        }
        self.workbench_state.focused_pane = pane;
    }

    /// Switch the context panel tab. Navigates to Alerts first if needed.
    pub fn set_alert_context_tab(&mut self, tab: AlertContextTab) {
        if self.screen != Screen::Alerts {
            self.switch_screen(Screen::Alerts);
        }
        self.workbench_state.active_context_tab = tab;
        self.workbench_state.context_scroll = 0;
    }

    /// Private helper: update alert status, mark read, refresh, notify.
    fn triage_set_status(&mut self, status: AlertStatus, msg: &str, ntype: NotificationType) {
        self.alerts_selected = self.workbench_state.selected_alert_index;
        let Some(a) = self.alerts_list.get(self.alerts_selected) else {
            self.set_notification("No alert selected".into(), NotificationType::Warning);
            return;
        };
        let result = self
            .db
            .update_alert_status(a.alert.id, status, None)
            .and_then(|()| self.db.mark_alert_read(a.alert.id, true));
        self.finish_triage_mutation(result, msg.into(), ntype);
    }

    /// Refresh triage views and report the storage result honestly.
    pub(crate) fn finish_triage_mutation(
        &mut self,
        result: anyhow::Result<()>,
        success_message: String,
        success_type: NotificationType,
    ) {
        self.refresh_alerts();
        match result {
            Ok(()) => self.set_notification(success_message, success_type),
            Err(error) => self.set_notification(
                format!("Alert update failed: {error}"),
                NotificationType::Error,
            ),
        }
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

    pub fn get_alert_workbench_bundle(&self, alert_id: i64) -> Option<AlertWorkbenchBundle> {
        let data = self.db.get_alert_report_data(alert_id).ok()??;

        let detail = AlertDetailViewModel {
            id: data.alert.id,
            title: data.alert.title.clone().unwrap_or_default(),
            content_snippet: data.alert.content_snippet.clone(),
            criticality: data.alert.criticality,
            effective_severity: data.alert.effective_severity(),
            status: data.alert.status,
            disposition: data.alert.disposition,
            read: data.alert.read,
            confidence_score: data.alert.confidence_score,
            owner: data.alert.owner.clone(),
            triage_notes: data.alert.triage_notes.clone(),
            detected_at: data.alert.detected_at,
            feed_name: data.feed_name.clone(),
            keyword_pattern: data.keyword_pattern.clone(),
            tags: data.tags.clone(),
            metadata_json: data.alert.metadata_json.clone(),
            acknowledged_at: data.alert.acknowledged_at,
            investigating_at: data.alert.investigating_at,
            escalated_at: data.alert.escalated_at,
            closed_at: data.alert.closed_at,
            closed_reason: data.alert.closed_reason.clone(),
        };

        let indicator_ids: Vec<i64> = data.indicators.iter().map(|ind| ind.id).collect();
        let enrichment_by_indicator = self
            .db
            .get_latest_enrichment_results_for_indicators(&indicator_ids)
            .ok()?;
        let indicators = data
            .indicators
            .iter()
            .map(|ind| {
                let enrichments = enrichment_by_indicator
                    .get(&ind.id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .map(|er| EnrichmentViewModel {
                        provider_id: er.provider_id,
                        status: er.status.clone(),
                        reputation: er.reputation.clone(),
                        score: er.score,
                        verdict: er.verdict.clone(),
                        summary: er.summary.clone(),
                        fetched_at: er.fetched_at,
                    })
                    .collect();
                IndicatorViewModel {
                    id: ind.id,
                    indicator_type: format!("{:?}", ind.indicator_type),
                    value: ind.value.clone(),
                    normalized_value: ind.normalized_value.clone(),
                    sighting_count: ind.sighting_count,
                    confidence_score: ind.confidence_score,
                    risk_score: ind.risk_score,
                    first_seen_at: ind.first_seen_at,
                    last_seen_at: ind.last_seen_at,
                    enrichments,
                }
            })
            .collect();

        let triage_events = data
            .triage_history
            .iter()
            .map(|ev| TriageEventViewModel {
                id: ev.id,
                event_type: ev.event_type.clone(),
                old_value: ev.old_value.clone(),
                new_value: ev.new_value.clone(),
                note: ev.note.clone(),
                actor: ev.actor.clone().unwrap_or_default(),
                created_at: ev.created_at,
            })
            .collect();

        Some(AlertWorkbenchBundle {
            detail,
            indicators,
            triage_events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::{AppConfig, Paths};
    use crate::db::{AlertCreate, Db, FeedCreate, KeywordCreate};
    use crate::types::{AlertStatus, Criticality, FeedType, NotificationType, Screen};
    use crate::ui::alerts::state::{AlertContextTab, AlertPane};
    use crate::ui::command_palette::AppAction;
    use std::path::PathBuf;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("threatdeck-app-{}-{}.db", name, std::process::id()))
    }

    fn setup_app(name: &str) -> (App, i64, i64) {
        let path = temp_db_path(name);
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
        db.init_schema().unwrap();
        let feed_id = db
            .create_feed(&FeedCreate {
                name: "Test".into(),
                url: "https://example.com/feed.xml".into(),
                feed_type: FeedType::Rss,
                enabled: true,
                interval_secs: 300,
                ..FeedCreate::default()
            })
            .unwrap();
        let keyword_id = db
            .create_keyword(&KeywordCreate {
                pattern: "breach".into(),
                criticality: Criticality::High,
                enabled: true,
                ..KeywordCreate::default()
            })
            .unwrap();
        let paths = Paths {
            config_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            db_file: path,
        };
        let app = App::new(db, AppConfig::default(), paths);
        (app, feed_id, keyword_id)
    }

    #[test]
    fn test_bundle_loads_selected_alert() {
        let (app, feed_id, keyword_id) = setup_app("bundle_loads");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Test Alert".into()),
                content_snippet: "test content".into(),
                criticality: Criticality::High,
                content_hash: "hash".into(),
                metadata_json: Some(r#"{"key":"value"}"#.into()),
            })
            .unwrap();
        let bundle = app.get_alert_workbench_bundle(alert_id);
        assert!(bundle.is_some());
        let bundle = bundle.unwrap();
        assert_eq!(bundle.detail.id, alert_id);
        assert_eq!(bundle.detail.title, "Test Alert");
        assert_eq!(bundle.detail.feed_name, "Test");
        assert!(bundle.detail.metadata_json.is_some());
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn test_bundle_returns_none_for_missing_alert() {
        let (app, _feed_id, _keyword_id) = setup_app("bundle_missing");
        assert!(app.get_alert_workbench_bundle(99999).is_none());
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn test_bundle_empty_indicators_when_none_exist() {
        let (app, feed_id, keyword_id) = setup_app("bundle_empty_iocs");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Test Alert".into()),
                content_snippet: "test content".into(),
                criticality: Criticality::High,
                content_hash: "hash".into(),
                metadata_json: None,
            })
            .unwrap();
        let bundle = app.get_alert_workbench_bundle(alert_id).unwrap();
        assert!(bundle.indicators.is_empty());
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn test_bundle_returns_indicators() {
        let (app, feed_id, keyword_id) = setup_app("bundle_iocs");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Test Alert".into()),
                content_snippet: "test content".into(),
                criticality: Criticality::High,
                content_hash: "hash".into(),
                metadata_json: None,
            })
            .unwrap();
        app.db
            .store_extracted_indicators(
                &[sentinel_ioc::ExtractedIndicator {
                    indicator_type: sentinel_ioc::IndicatorType::Cve,
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
        let bundle = app.get_alert_workbench_bundle(alert_id).unwrap();
        assert_eq!(bundle.indicators.len(), 1);
        assert_eq!(bundle.indicators[0].value, "CVE-2025-12345");
        assert!(bundle.indicators[0].enrichments.is_empty());
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn test_bundle_returns_triage_history() {
        let (app, feed_id, keyword_id) = setup_app("bundle_triage");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Test Alert".into()),
                content_snippet: "test content".into(),
                criticality: Criticality::High,
                content_hash: "hash".into(),
                metadata_json: None,
            })
            .unwrap();
        app.db
            .update_alert_status(alert_id, AlertStatus::Acknowledged, Some("acknowledging"))
            .unwrap();
        let bundle = app.get_alert_workbench_bundle(alert_id).unwrap();
        assert_eq!(bundle.triage_events.len(), 1);
        assert_eq!(bundle.triage_events[0].event_type, "status_changed");
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn test_bundle_empty_enrichment_when_none_exist() {
        let (app, feed_id, keyword_id) = setup_app("bundle_empty_enrich");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Test Alert".into()),
                content_snippet: "test content".into(),
                criticality: Criticality::High,
                content_hash: "hash".into(),
                metadata_json: None,
            })
            .unwrap();
        app.db
            .store_extracted_indicators(
                &[sentinel_ioc::ExtractedIndicator {
                    indicator_type: sentinel_ioc::IndicatorType::Cve,
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
        let bundle = app.get_alert_workbench_bundle(alert_id).unwrap();
        assert!(!bundle.indicators.is_empty());
        assert!(bundle.indicators[0].enrichments.is_empty());
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    // ── Phase 5: Command palette dispatch tests ───────────────────────────────

    #[test]
    fn palette_focus_list_pane() {
        let (mut app, _, _) = setup_app("pal_focus_list");
        app.screen = Screen::Alerts;
        app.workbench_state.focused_pane = AlertPane::Details;
        app.handle_app_action(AppAction::AlertFocusList);
        assert_eq!(app.workbench_state.focused_pane, AlertPane::List);
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_focus_details_pane() {
        let (mut app, _, _) = setup_app("pal_focus_details");
        app.screen = Screen::Alerts;
        app.handle_app_action(AppAction::AlertFocusDetails);
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Details);
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_focus_context_pane() {
        let (mut app, _, _) = setup_app("pal_focus_ctx");
        app.screen = Screen::Alerts;
        app.handle_app_action(AppAction::AlertFocusContext);
        assert_eq!(app.workbench_state.focused_pane, AlertPane::Context);
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_switch_context_tab_iocs() {
        let (mut app, _, _) = setup_app("pal_tab_iocs");
        app.screen = Screen::Alerts;
        app.workbench_state.active_context_tab = AlertContextTab::Metadata;
        app.handle_app_action(AppAction::AlertContextIocs);
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::Iocs
        );
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_switch_context_tab_metadata() {
        let (mut app, _, _) = setup_app("pal_tab_meta");
        app.screen = Screen::Alerts;
        app.handle_app_action(AppAction::AlertContextMetadata);
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::Metadata
        );
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_switch_context_tab_history() {
        let (mut app, _, _) = setup_app("pal_tab_hist");
        app.screen = Screen::Alerts;
        app.handle_app_action(AppAction::AlertContextHistory);
        assert_eq!(
            app.workbench_state.active_context_tab,
            AlertContextTab::TriageHistory
        );
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_triage_acknowledge_selected_alert() {
        let (mut app, feed_id, keyword_id) = setup_app("pal_ack");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Pal Ack Test".into()),
                content_snippet: "content".into(),
                criticality: Criticality::High,
                content_hash: "pal-ack".into(),
                metadata_json: None,
            })
            .unwrap();
        app.refresh_alerts();
        app.screen = Screen::Alerts;
        // Find the created alert by ID and select it
        let idx = app
            .workbench_state
            .alerts
            .iter()
            .position(|a| a.id == alert_id)
            .expect("created alert should be in the list");
        app.workbench_state.selected_alert_index = idx;
        app.handle_app_action(AppAction::AlertTriageAcknowledge);
        let (msg, ntype) = app.notification.as_ref().unwrap();
        assert_eq!(*ntype, NotificationType::Success);
        assert!(msg.contains("acknowledged"));
        // Verify status changed in DB
        let alert = app.db.get_alert(alert_id).unwrap().unwrap();
        assert_eq!(alert.status, AlertStatus::Acknowledged);
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_triage_acknowledge_no_alert_warns() {
        let (mut app, _, _) = setup_app("pal_ack_none");
        app.screen = Screen::Alerts;
        app.workbench_state.selected_alert_index = 999;
        app.alerts_selected = 999;
        app.handle_app_action(AppAction::AlertTriageAcknowledge);
        let (msg, ntype) = app.notification.as_ref().unwrap();
        assert_eq!(*ntype, NotificationType::Warning);
        assert!(msg.contains("No alert selected"));
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn triage_reopen_refuses_non_closed_alert() {
        let (mut app, feed_id, keyword_id) = setup_app("reopen_open");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Still open".into()),
                content_snippet: "content".into(),
                criticality: Criticality::High,
                content_hash: "reopen-open".into(),
                metadata_json: None,
            })
            .unwrap();
        app.refresh_alerts();
        app.workbench_state.selected_alert_index = app
            .workbench_state
            .alerts
            .iter()
            .position(|a| a.id == alert_id)
            .unwrap();

        app.triage_reopen();

        assert_eq!(
            app.db.get_alert(alert_id).unwrap().unwrap().status,
            AlertStatus::New
        );
        let (message, kind) = app.notification.as_ref().unwrap();
        assert_eq!(*kind, NotificationType::Warning);
        assert_eq!(message, "Alert is not closed");
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn triage_status_db_error_shows_error_notification() {
        let (mut app, feed_id, keyword_id) = setup_app("triage_db_error");
        let alert_id = app
            .db
            .create_alert(&AlertCreate {
                feed_id,
                keyword_id,
                title: Some("Unwritable alert".into()),
                content_snippet: "content".into(),
                criticality: Criticality::High,
                content_hash: "triage-db-error".into(),
                metadata_json: None,
            })
            .unwrap();
        app.refresh_alerts();
        app.workbench_state.selected_alert_index = app
            .workbench_state
            .alerts
            .iter()
            .position(|a| a.id == alert_id)
            .unwrap();
        let connection = rusqlite::Connection::open(&app.paths.db_file).unwrap();
        connection
            .execute(
                "CREATE TRIGGER reject_alert_update BEFORE UPDATE ON alerts
                 BEGIN SELECT RAISE(FAIL, 'forced update failure'); END;",
                [],
            )
            .unwrap();
        drop(connection);

        app.triage_acknowledge();

        assert_eq!(
            app.db.get_alert(alert_id).unwrap().unwrap().status,
            AlertStatus::New
        );
        let (message, kind) = app.notification.as_ref().unwrap();
        assert_eq!(*kind, NotificationType::Error);
        assert!(message.contains("forced update failure"));
        let _ = std::fs::remove_file(&app.paths.db_file);
    }

    #[test]
    fn palette_export_via_action() {
        let (mut app, _, _) = setup_app("pal_export");
        app.handle_app_action(AppAction::AlertExportSelectedMarkdown);
        let (_, ntype) = app.notification.as_ref().unwrap();
        assert_eq!(*ntype, NotificationType::Success);
        let _ = std::fs::remove_file(&app.paths.db_file);
    }
}
