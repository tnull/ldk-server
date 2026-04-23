//! Feature-gated glue between [`crate::util::config::LogSanitizerConfig`] and the
//! [`log_sanitizer`] crate.
//!
//! When the `privacy-filter` Cargo feature is enabled and the config file contains a
//! `[log.sanitizer]` section, [`install_sanitizer`] wraps the `ServerLogger` in a
//! sanitizing adapter before registering the global `log` sink. When the feature is
//! disabled, a stub implementation loudly refuses to start if the config section is
//! present.

use std::io;
use std::sync::Arc;

use crate::util::config::LogSanitizerConfig;
use crate::util::logger::ServerLogger;

#[cfg(feature = "privacy-filter")]
use crate::util::config::{LogSanitizerKind, LogSanitizerOnOverflow};

/// Install the global `log` sink, wrapping `ServerLogger` in a sanitizing adapter when
/// the config says to and the feature is compiled in.
///
/// Returns an `Arc<ServerLogger>` so the caller (main.rs) can still hold on to the
/// underlying logger for SIGHUP rotation.
pub fn install_sanitizer(
	server_logger: Arc<ServerLogger>, sanitizer_cfg: Option<&LogSanitizerConfig>,
) -> Result<(), io::Error> {
	match sanitizer_cfg {
		None => ServerLogger::install(
			Arc::clone(&server_logger),
			Arc::clone(&server_logger).as_boxed_log(),
		),
		Some(cfg) => install_with_sanitizer(server_logger, cfg),
	}
}

#[cfg(not(feature = "privacy-filter"))]
fn install_with_sanitizer(
	_server_logger: Arc<ServerLogger>, _cfg: &LogSanitizerConfig,
) -> Result<(), io::Error> {
	Err(io::Error::new(
		io::ErrorKind::InvalidInput,
		"`[log.sanitizer]` is configured but `ldk-server` was built without the \
		 `privacy-filter` Cargo feature. Rebuild with `--features privacy-filter` or \
		 remove the `[log.sanitizer]` section from your config.",
	))
}

#[cfg(feature = "privacy-filter")]
fn install_with_sanitizer(
	server_logger: Arc<ServerLogger>, cfg: &LogSanitizerConfig,
) -> Result<(), io::Error> {
	use log_sanitizer::{
		AsyncSanitizingLogger, OnnxSanitizer, OverflowPolicy, RegexSanitizer, SanitizingLogger,
	};

	let inner = Arc::clone(&server_logger).as_boxed_log();

	let boxed_log: Box<dyn log::Log + Send + Sync> = match cfg.kind {
		LogSanitizerKind::Regex => Box::new(
			SanitizingLogger::new(inner, RegexSanitizer::lightning_defaults())
				.with_min_level(cfg.min_level),
		),
		LogSanitizerKind::Onnx => {
			let model_dir = cfg.model_dir.as_deref().ok_or_else(|| {
				io::Error::new(
					io::ErrorKind::InvalidInput,
					"[log.sanitizer] kind = \"onnx\" requires `model_dir` to be set",
				)
			})?;
			let onnx = OnnxSanitizer::from_dir(model_dir).map_err(|e| {
				io::Error::new(
					io::ErrorKind::InvalidInput,
					format!("failed to load privacy-filter model from {model_dir:?}: {e}"),
				)
			})?;
			let overflow = match cfg.on_overflow {
				LogSanitizerOnOverflow::Drop => OverflowPolicy::Drop,
				LogSanitizerOnOverflow::Block => OverflowPolicy::Block,
				LogSanitizerOnOverflow::FallbackRegex => OverflowPolicy::Fallback(
					std::sync::Arc::new(RegexSanitizer::lightning_defaults()),
				),
			};
			let async_logger = AsyncSanitizingLogger::new(
				InnerBox(inner),
				onnx,
				/* capacity = */ 128,
				overflow,
			)
			.with_min_level(cfg.min_level);
			Box::new(async_logger)
		},
	};

	ServerLogger::install(server_logger, boxed_log)
}

/// Helper type: `AsyncSanitizingLogger::new` requires `L: Log + 'static`, and
/// `Box<dyn Log + Send + Sync>` satisfies that only when re-wrapped as a concrete type.
#[cfg(feature = "privacy-filter")]
struct InnerBox(Box<dyn log::Log + Send + Sync>);

#[cfg(feature = "privacy-filter")]
impl log::Log for InnerBox {
	fn enabled(&self, m: &log::Metadata) -> bool {
		self.0.enabled(m)
	}
	fn log(&self, record: &log::Record) {
		self.0.log(record);
	}
	fn flush(&self) {
		self.0.flush();
	}
}
