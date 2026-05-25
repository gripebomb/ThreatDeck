use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ReportError {
    AlertNotFound(i64),
    CaseNotFound(i64),
    IndicatorNotFound(i64),
    ExportPathExists(PathBuf),
    InvalidExportPath(PathBuf),
    CouldNotCreateDirectory(PathBuf),
    CouldNotWriteFile(PathBuf, String),
    StorageError(String),
    InvalidFilter(String),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::AlertNotFound(id) => write!(f, "Alert {id} was not found."),
            ReportError::CaseNotFound(id) => write!(f, "Case {id} was not found."),
            ReportError::IndicatorNotFound(id) => {
                write!(f, "Indicator {id} was not found.")
            }
            ReportError::ExportPathExists(path) => {
                write!(
                    f,
                    "Export path already exists: {}. Use --overwrite to replace it.",
                    path.display()
                )
            }
            ReportError::InvalidExportPath(path) => {
                write!(f, "Invalid export path: {}", path.display())
            }
            ReportError::CouldNotCreateDirectory(path) => {
                write!(f, "Could not create export directory: {}", path.display())
            }
            ReportError::CouldNotWriteFile(path, reason) => {
                write!(
                    f,
                    "Could not write report file: {} - {}",
                    path.display(),
                    reason
                )
            }
            ReportError::StorageError(msg) => write!(f, "Storage error: {msg}"),
            ReportError::InvalidFilter(msg) => write!(f, "Invalid filter: {msg}"),
        }
    }
}

impl std::error::Error for ReportError {}
