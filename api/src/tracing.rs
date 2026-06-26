use std::{io, net::IpAddr, sync::OnceLock, time::Duration};

use anyhow::Ok;
use opentelemetry::{
    KeyValue,
    global::{self, BoxedTracer},
    metrics::MeterProvider,
};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    logs::{SdkLoggerProvider, SimpleLogProcessor},
    metrics::SdkMeterProvider,
    trace::{BatchConfigBuilder, SdkTracerProvider},
};
use opentelemetry_stdout::LogExporter;
use tower_http::trace;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, layer},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

pub struct TelemetryGuard {
    pub tracer_provider: SdkTracerProvider,
    pub logger_provider: SdkLoggerProvider,
    pub metrics_provider: SdkMeterProvider,
}

impl TelemetryGuard {
    pub fn shutdown(&self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("Error shutting down tracer provider: {e}")
        }
        if let Err(e) = self.logger_provider.shutdown() {
            eprintln!("Error shutting down logger provider: {e}")
        }
        if let Err(e) = self.metrics_provider.shutdown() {
            eprintln!("Error shutting down metrics provider {e}")
        }
    }
}

pub fn init_telemetry(service_name: &str, otlp_endpoint: &str) -> anyhow::Result<TelemetryGuard> {
    let resource = build_resource(service_name, "development");

    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(10))
        .build()?;
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(trace_exporter)
        .with_resource(resource.clone())
        .build();
    // let batch_config = BatchConfigBuilder::default()
    //     .with_max_queue_size(2048)
    //     .with_scheduled_delay(Duration::from_secs(5)) // Set to 5 in development
    //     .with_max_export_batch_size(512)
    //     .build();
    global::set_tracer_provider(tracer_provider.clone());

    // let log_exporter = LogExporter::builder()
    //     .with_tonic()
    //     .with_endpoint(otlp_endpoint)
    //     .with_timeout(Duration::from_secs(10))
    //     .build()?;

    let logger_provider = SdkLoggerProvider::builder()
        .with_simple_exporter(LogExporter::default())
        .with_resource(resource.clone())
        .build();

    let meter_export = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .with_timeout(Duration::from_secs(10))
        .build()?;

    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(meter_export)
        .with_resource(resource)
        .build();

    global::set_meter_provider(meter_provider.clone());

    let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);
    let tracer = global::tracer(service_name.to_string());
    let telemetry_layer = OpenTelemetryLayer::new(tracer);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=debug"));

    let fmt_layer = fmt::layer()
        .json()
        .with_timer(fmt::time::UtcTime::rfc_3339());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(telemetry_layer)
        .with(otel_log_layer)
        .with(fmt_layer)
        .init();

    tracing::info!(
        service = %service_name,
        endpoint = %otlp_endpoint,
        "Telemetry initialized with OTLP"
    );

    Ok(TelemetryGuard {
        tracer_provider,
        logger_provider,
        metrics_provider: meter_provider,
    })
}

fn build_resource(service_name: &str, environment: &str) -> Resource {
    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .with_attribute(KeyValue::new("service.version", "1.0.0"))
        .with_attribute(KeyValue::new("service.namespace", "development"))
        .with_attribute(KeyValue::new("service.instance.id", 1))
        .with_attribute(KeyValue::new("environment", "production"))
        .with_attribute(KeyValue::new("process.runtime.name", "rustc".to_string()))
        .with_attribute(KeyValue::new(
            "process.runtime.version",
            env!("CARGO_PKG_RUST_VERSION"),
        ))
        .build();
    resource
}

pub fn get_tracer(service_name: String) -> &'static BoxedTracer {
    static TRACER: OnceLock<BoxedTracer> = OnceLock::new();
    TRACER.get_or_init(|| global::tracer(service_name))
}
