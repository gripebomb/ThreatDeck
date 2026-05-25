use crate::types::Screen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    OpenDashboard,
    OpenFeeds,
    OpenAlerts,
    OpenKeywords,
    OpenTags,
    OpenIndicators,
    OpenSettings,
    OpenHelp,

    Refresh,
    Quit,

    FeedAdd,
    FeedEditSelected,
    FeedCheckSelected,
    FeedCheckAll,
    FeedEnableSelected,
    FeedDisableSelected,
    FeedHealth,

    AlertShowUnread,
    AlertShowCritical,
    AlertOpenSelected,
    AlertMarkSelectedRead,
    AlertMarkSelectedUnread,
    AlertMarkVisibleRead,
    AlertExportSelectedMarkdown,
    AlertExportVisibleMarkdown,
    FeedHealthExportMarkdown,

    KeywordAdd,
    KeywordEditSelected,
    KeywordTestSelected,
    KeywordEnableSelected,
    KeywordDisableSelected,

    DoctorRun,
    DoctorTor,
    DoctorDatabase,
    DoctorNotifications,

    NotifyTestDiscord,
    NotifyTestWebhook,
    NotifyTestEmail,
    NotifyOpenSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGroup {
    Navigation,
    Feed,
    Alert,
    Keyword,
    Doctor,
    Notification,
    System,
}

impl CommandGroup {
    pub fn label(self) -> &'static str {
        match self {
            CommandGroup::Navigation => "Nav",
            CommandGroup::Feed => "Feed",
            CommandGroup::Alert => "Alert",
            CommandGroup::Keyword => "Keyword",
            CommandGroup::Doctor => "Doctor",
            CommandGroup::Notification => "Notify",
            CommandGroup::System => "System",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAvailability {
    Always,
    WhenFeedSelected,
    WhenAlertSelected,
    WhenKeywordSelected,
    WhenNotificationRouteConfigured(NotificationRouteKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationRouteKind {
    Discord,
    Webhook,
    Email,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    Navigate(Screen),
    OpenModal(ModalKind),
    Dispatch(AppAction),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Help,
    Confirm,
    EditFeed,
    EditKeyword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Refresh,
    FeedAdd,
    FeedEditSelected,
    FeedCheckSelected,
    FeedCheckAll,
    FeedEnableSelected,
    FeedDisableSelected,
    AlertShowUnread,
    AlertShowCritical,
    AlertMarkSelectedRead,
    AlertMarkSelectedUnread,
    AlertMarkVisibleRead,
    AlertExportSelectedMarkdown,
    AlertExportVisibleMarkdown,
    FeedHealthExportMarkdown,

    KeywordAdd,
    KeywordEditSelected,
    KeywordTestSelected,
    DoctorRun,
    DoctorTor,
    DoctorDatabase,
    DoctorNotifications,
    NotifyTestDiscord,
    NotifyTestWebhook,
    NotifyTestEmail,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub id: CommandId,
    pub canonical: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub group: CommandGroup,
    pub aliases: &'static [&'static str],
    pub keywords: &'static [&'static str],
    pub availability: CommandAvailability,
    pub action: CommandAction,
}
