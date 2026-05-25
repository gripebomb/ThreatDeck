pub mod error;
pub mod filenames;
pub mod markdown;
pub mod types;

pub use error::ReportError;
pub use filenames::{ensure_safe_path, generate_filename, sanitize_filename};
pub use markdown::{
    escape_table_cell, fenced_code_block, redact_sensitive, render_alert_collection_report,
    render_alert_report, render_daily_summary_report, render_feed_health_report,
};
pub use types::*;
