use std::any::type_name;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

static LOGGER_INIT: OnceLock<()> = OnceLock::new();
static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

#[derive(Debug)]
pub enum LoggerInitError {
    Io(std::io::Error),
    SetGlobalDefault(tracing::subscriber::SetGlobalDefaultError),
}

impl Display for LoggerInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to prepare logging directory: {err}"),
            Self::SetGlobalDefault(err) => write!(f, "failed to initialize global logger: {err}"),
        }
    }
}

impl std::error::Error for LoggerInitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::SetGlobalDefault(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for LoggerInitError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tracing::subscriber::SetGlobalDefaultError> for LoggerInitError {
    fn from(value: tracing::subscriber::SetGlobalDefaultError) -> Self {
        Self::SetGlobalDefault(value)
    }
}

pub trait HasLogger {
    fn logger_name(&self) -> String {
        short_type_name::<Self>()
    }

    fn log_folder(&self) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".logs")
    }

    fn log_file_prefix(&self) -> String {
        format!("{}.log", self.logger_name())
    }

    fn init_logger(&self) -> Result<(), LoggerInitError> {
        init_global_logger(self.log_folder(), self.log_file_prefix())?;
        tracing::debug!("{}({}) initialized", "RustLogger", self.logger_name());
        Ok(())
    }

    fn debug(&self, message: impl AsRef<str>) {
        tracing::debug!("[{}] {}", self.logger_name(), message.as_ref());
    }

    fn info(&self, message: impl AsRef<str>) {
        tracing::info!("[{}] {}", self.logger_name(), message.as_ref());
    }

    fn warn(&self, message: impl AsRef<str>) {
        tracing::warn!("[{}] {}", self.logger_name(), message.as_ref());
    }

    fn error(&self, message: impl AsRef<str>) {
        tracing::error!("[{}] {}", self.logger_name(), message.as_ref());
    }
}

fn init_global_logger(log_dir: PathBuf, log_file_prefix: String) -> Result<(), LoggerInitError> {
    if LOGGER_INIT.get().is_some() {
        return Ok(());
    }

    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(log_dir, log_file_prefix);
    let (non_blocking_file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy()
    });

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_timer(SystemTime)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_target(true)
        .with_ansi(true)
        .with_writer(std::io::stdout);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_timer(SystemTime)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_target(true)
        .with_ansi(false)
        .with_writer(non_blocking_file_writer);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer);

    tracing::subscriber::set_global_default(subscriber)?;
    let _ = FILE_GUARD.set(file_guard);
    let _ = LOGGER_INIT.set(());

    Ok(())
}

fn short_type_name<T: ?Sized>() -> String {
    type_name::<T>()
        .rsplit("::")
        .next()
        .unwrap_or("Unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::HasLogger;

    struct Sample;

    impl HasLogger for Sample {}

    #[test]
    fn default_logger_name_uses_struct_name() {
        let sample = Sample;
        assert_eq!(sample.logger_name(), "Sample");
        assert_eq!(sample.log_file_prefix(), "Sample.log");
    }
}
