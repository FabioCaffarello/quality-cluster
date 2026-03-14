use std::fmt;

#[derive(Debug)]
pub enum CliError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Command { message: String },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Io(e) => write!(f, "io error: {e}"),
            CliError::Json(e) => write!(f, "json error: {e}"),
            CliError::Command { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
        let cli_err = CliError::Io(io_err);
        let msg = cli_err.to_string();
        assert!(msg.starts_with("io error:"), "got: {msg}");
        assert!(msg.contains("file gone"));
    }

    #[test]
    fn display_json_error() {
        let json_err: serde_json::Error = serde_json::from_str::<serde_json::Value>("{bad}").unwrap_err();
        let cli_err = CliError::Json(json_err);
        let msg = cli_err.to_string();
        assert!(msg.starts_with("json error:"), "got: {msg}");
    }

    #[test]
    fn display_command_error() {
        let cli_err = CliError::Command {
            message: "something broke".to_string(),
        };
        assert_eq!(cli_err.to_string(), "something broke");
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
        let cli_err: CliError = io_err.into();
        assert!(matches!(cli_err, CliError::Io(_)));
    }

    #[test]
    fn from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("!!!").unwrap_err();
        let cli_err: CliError = json_err.into();
        assert!(matches!(cli_err, CliError::Json(_)));
    }

    #[test]
    fn cli_error_is_error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(CliError::Command {
            message: "test".into(),
        });
        assert_eq!(err.to_string(), "test");
    }
}
