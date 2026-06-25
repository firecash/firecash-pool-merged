//! Telemetry wiring for the katpool runtime.
//!
//! A single [`init`] call installs the process-wide `tracing` subscriber that
//! every binary uses for structured logging and (optionally) distributed
//! tracing. It composes up to three layers over a [`tracing_subscriber`]
//! registry:
//!
//! 1. An [`EnvFilter`] sourced from `RUST_LOG` (falling back to a configurable
//!    default directive), so verbosity is tunable without a rebuild.
//! 2. A formatting layer that emits either machine-readable **JSON** (one
//!    object per event, suited to Loki/`journald` ingestion) or human-readable
//!    text. Both carry event fields — including the `correlation_id` that the
//!    domain attaches to every `PoolEvent` — so a
//!    single share or payout can be traced end-to-end across async tasks.
//! 3. An optional OpenTelemetry OTLP/gRPC export layer
//!    ([`tracing_opentelemetry`]) that ships spans to a collector (Tempo, per
//!    ADR-0004). It is wired only when an endpoint is configured; until the
//!    self-hosted LGTM stack exists the runtime simply leaves it off and pays
//!    no export cost.
//!
//! [`init`] returns a [`TelemetryGuard`]; hold it for the life of the process.
//! Dropping it flushes and shuts down the tracer provider so no spans are lost
//! on a clean exit.
//!
//! ## Treasury redaction
//!
//! This crate never logs secret material itself. Callers must redact
//! semi-sensitive identifiers (wallet/treasury addresses) before emitting them
//! via `katpool_domain::redact`; treasury *key* material is structurally
//! unloggable (`katpool_secrets::TreasurySecret` has no `Debug`/`Display`).

#![cfg_attr(not(test), warn(missing_docs))]
// Tests use `unwrap`/assertion macros that clippy treats like `panic`;
// relaxing those under `cfg(test)` keeps the strict policy on production paths.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::str::FromStr;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig as _};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

/// Crate version constant.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default tracing directive used when `RUST_LOG` is unset or unparsable.
pub const DEFAULT_LOG_DIRECTIVE: &str = "info";

/// Default OpenTelemetry service name when none is supplied.
pub const DEFAULT_SERVICE_NAME: &str = "katpool";

/// Errors raised while installing telemetry. All are fatal at startup.
#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    /// Building the OTLP span exporter (endpoint/TLS/transport) failed.
    #[error("building OTLP span exporter: {0}")]
    Exporter(#[from] opentelemetry_otlp::ExporterBuildError),
    /// A global `tracing` subscriber was already installed in this process.
    #[error("installing global tracing subscriber: {0}")]
    Install(#[from] tracing_subscriber::util::TryInitError),
}

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// One JSON object per event — for Loki/`journald` structured ingestion.
    Json,
    /// Human-readable single-line text — for local development and `journalctl`.
    Text,
}

impl FromStr for LogFormat {
    type Err = UnknownLogFormat;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "text" | "plain" | "pretty" => Ok(Self::Text),
            other => Err(UnknownLogFormat(other.to_owned())),
        }
    }
}

/// Returned when [`LogFormat::from_str`] cannot parse its input.
#[derive(Debug, thiserror::Error)]
#[error("unknown log format {0:?} (expected `json` or `text`)")]
pub struct UnknownLogFormat(String);

/// Telemetry configuration, resolved once at startup.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// `service.name` resource attribute reported on exported spans.
    pub service_name: String,
    /// Log output format.
    pub log_format: LogFormat,
    /// OTLP/gRPC collector endpoint (e.g. `http://tempo:4317`). `None` disables
    /// span export entirely.
    pub otlp_endpoint: Option<String>,
    /// Directive applied when `RUST_LOG` is unset/unparsable.
    pub default_directive: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: DEFAULT_SERVICE_NAME.to_owned(),
            log_format: LogFormat::Text,
            otlp_endpoint: None,
            default_directive: DEFAULT_LOG_DIRECTIVE.to_owned(),
        }
    }
}

impl TelemetryConfig {
    /// Resolve configuration from the process environment.
    ///
    /// - `service.name`: `OTEL_SERVICE_NAME`, else `service_name` argument.
    /// - `KATPOOL_LOG_FORMAT`: `json` | `text` (default `text`; an
    ///   unrecognised value falls back to `text` rather than aborting startup,
    ///   since logging must never be the reason a node fails to boot).
    /// - `KATPOOL_OTLP_ENDPOINT`: collector endpoint; empty/unset = export off.
    #[must_use]
    pub fn from_env(service_name: impl Into<String>) -> Self {
        let service_name = std::env::var("OTEL_SERVICE_NAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| service_name.into());

        let log_format = std::env::var("KATPOOL_LOG_FORMAT")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(LogFormat::Text);

        let otlp_endpoint = std::env::var("KATPOOL_OTLP_ENDPOINT")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());

        Self {
            service_name,
            log_format,
            otlp_endpoint,
            default_directive: DEFAULT_LOG_DIRECTIVE.to_owned(),
        }
    }
}

/// Flushes and shuts down telemetry on drop. Hold for the process lifetime.
#[must_use = "dropping the guard immediately tears telemetry back down"]
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            // Best-effort flush of any batched spans; nothing actionable
            // remains if the exporter is already gone during shutdown.
            let _ = provider.shutdown();
        }
    }
}

/// Install the global `tracing` subscriber per `config`.
///
/// Call exactly once, before spawning any subsystem. Returns a
/// [`TelemetryGuard`] that must be kept alive for the program's duration.
///
/// # Errors
///
/// Returns [`TelemetryError`] if the OTLP exporter cannot be built or a global
/// subscriber was already installed in this process.
pub fn init(config: &TelemetryConfig) -> Result<TelemetryGuard, TelemetryError> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.default_directive));

    let fmt_layer = match config.log_format {
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .boxed(),
        LogFormat::Text => tracing_subscriber::fmt::layer().with_target(true).boxed(),
    };

    let (otel_layer, provider) = match config.otlp_endpoint.as_deref() {
        Some(endpoint) => {
            let exporter = SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .build()?;
            let resource = Resource::builder()
                .with_service_name(config.service_name.clone())
                .build();
            let provider = SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(resource)
                .build();
            let tracer = provider.tracer(DEFAULT_SERVICE_NAME);
            opentelemetry::global::set_tracer_provider(provider.clone());
            let layer = tracing_opentelemetry::layer().with_tracer(tracer);
            (Some(layer), Some(provider))
        }
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()?;

    Ok(TelemetryGuard { provider })
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_SERVICE_NAME, LogFormat, TelemetryConfig};

    #[test]
    fn log_format_parses_known_aliases() {
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("JSON".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("text".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!(" Pretty ".parse::<LogFormat>().unwrap(), LogFormat::Text);
    }

    #[test]
    fn log_format_rejects_unknown() {
        assert!("yaml".parse::<LogFormat>().is_err());
    }

    #[test]
    fn default_is_text_no_export() {
        let cfg = TelemetryConfig::default();
        assert_eq!(cfg.log_format, LogFormat::Text);
        assert!(cfg.otlp_endpoint.is_none());
        assert_eq!(cfg.service_name, DEFAULT_SERVICE_NAME);
    }
}
