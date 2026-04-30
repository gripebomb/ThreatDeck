use crossterm::event::{KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::config::{AppConfig, Paths};
use crate::db::{AlertFilter, Db};
use crate::theme::{get_runtime_theme, Theme};
use crate::types::*;
use crate::ui;

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

    // Feeds
    pub feeds_list: Vec<FeedWithTags>,
    pub feeds_selected: usize,
    pub feeds_filter: String,
    pub feeds_show_form: bool,
    pub feeds_form: FeedForm,
    pub feeds_form_edit_id: Option<i64>,
    pub feeds_detail_view: bool,
    pub feeds_sort: usize,

    // Alerts
    pub alerts_list: Vec<AlertWithMeta>,
    pub alerts_selected: usize,
    pub alerts_filter: String,
    pub alerts_filter_criticality: Option<Criticality>,
    pub alerts_filter_unread_only: bool,
    pub alerts_detail_view: bool,
    pub alerts_bulk_mode: bool,
    pub alerts_selected_bulk: HashSet<i64>,

    // Articles
    pub articles_list: Vec<FeedItemWithFeed>,
    pub articles_selected: usize,
    pub articles_filter: String,
    pub articles_unread_only: bool,
    pub articles_reader: bool,
    pub articles_scroll: u16,

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
    pub settings_notif_form: bool,
    pub settings_notif_form_data: NotificationForm,
    pub settings_notif_form_edit_id: Option<i64>,
    pub settings_cleanup_preview: Option<u64>,
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
            feeds_list: Vec::new(),
            feeds_selected: 0,
            feeds_filter: String::new(),
            feeds_show_form: false,
            feeds_form: FeedForm::default(),
            feeds_form_edit_id: None,
            feeds_detail_view: false,
            feeds_sort: 0,
            alerts_list: Vec::new(),
            alerts_selected: 0,
            alerts_filter: String::new(),
            alerts_filter_criticality: None,
            alerts_filter_unread_only: false,
            alerts_detail_view: false,
            alerts_bulk_mode: false,
            alerts_selected_bulk: HashSet::new(),
            articles_list: Vec::new(),
            articles_selected: 0,
            articles_filter: String::new(),
            articles_unread_only: false,
            articles_reader: false,
            articles_scroll: 0,
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
            settings_notif_form: false,
            settings_notif_form_data: NotificationForm::default(),
            settings_notif_form_edit_id: None,
            settings_cleanup_preview: None,
        };
        app.refresh_dashboard();
        app.refresh_feeds();
        app.refresh_alerts();
        app.refresh_articles();
        app.refresh_keywords();
        app.refresh_tags();
        app.refresh_logs();
        app.refresh_settings();
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

        // When in Typing mode, all keystrokes go to the active form (except Esc to exit Typing mode)
        if self.input_mode == InputMode::Typing {
            if key.code == KeyCode::Esc {
                self.input_mode = InputMode::Normal;
                return;
            }
            // Delegate to the screen-specific form handler
            self.handle_screen_key(key);
            return;
        }

        // Normal mode global shortcuts
        match key.code {
            KeyCode::Char('q') if !self.in_form() => self.running = false,
            KeyCode::Char('1') => self.switch_screen(Screen::Dashboard),
            KeyCode::Char('2') => self.switch_screen(Screen::Feeds),
            KeyCode::Char('3') => self.switch_screen(Screen::Alerts),
            KeyCode::Char('4') => self.switch_screen(Screen::Articles),
            KeyCode::Char('5') => self.switch_screen(Screen::Keywords),
            KeyCode::Char('6') => self.switch_screen(Screen::Tags),
            KeyCode::Char('7') => self.switch_screen(Screen::Logs),
            KeyCode::Char('8') => self.switch_screen(Screen::Settings),
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = true,
            KeyCode::Char('/') => self.start_filter(),
            KeyCode::Esc => self.handle_esc(),
            _ => self.handle_screen_key(key),
        }
    }

    /// Returns true if a data-entry form (not a test/assignment overlay) is active.
    /// Used to decide whether global shortcuts like 'q' should be suppressed.
    fn in_form(&self) -> bool {
        match self.screen {
            Screen::Feeds => self.feeds_show_form,
            Screen::Keywords => self.keywords_show_form,
            Screen::Tags => self.tags_show_form,
            Screen::Settings => self.settings_notif_form,
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

        match crate::feed::FeedManager::fetch_feed(&feed, template) {
            Ok(result) => {
                let item_count = result.items.len();
                let keywords = self.db.list_keywords(true).unwrap_or_default();
                match crate::alert::AlertEngine::process_feed_result(
                    &self.db, &feed, &result, &keywords,
                ) {
                    Ok(alerts) => {
                        let _ = self.db.update_feed_health(
                            feed.id,
                            true,
                            None,
                            Some(result.content_hash.as_str()),
                        );
                        let _ = self.db.add_health_log(feed.id, FeedStatus::Healthy, None);
                        let _ = self
                            .db
                            .prune_health_logs(feed.id, self.config.max_health_log_entries);
                        self.refresh_feeds();
                        self.refresh_articles();
                        self.refresh_alerts();
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
                        self.set_notification(
                            format!("Feed processed but storing alerts/items failed: {}", e),
                            NotificationType::Error,
                        );
                    }
                }
            }
            Err(e) => {
                let message = e.to_string();
                let _ = self
                    .db
                    .update_feed_health(feed.id, false, Some(message.as_str()), None);
                let _ = self
                    .db
                    .add_health_log(feed.id, FeedStatus::Error, Some(message.as_str()));
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

    pub fn refresh_settings(&mut self) {
        if let Ok(notifs) = self.db.list_notifications() {
            self.settings_notifications = notifs;
        }
        self.settings_retention_days = self.config.alert_retention_days;
        self.settings_theme_name = self.config.theme.clone();
    }
}
