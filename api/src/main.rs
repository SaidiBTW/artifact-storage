use api::{
    routes::create_router,
    tracing::{TelemetryGuard, init_telemetry},
    types::app_state::AppState,
};
use dotenvy::dotenv;
use opentelemetry::global::{self, BoxedTracer};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::{
    logs::SdkLoggerProvider, metrics::SdkMeterProvider, trace::SdkTracerProvider,
};
use opentelemetry_stdout::{LogExporter, MetricExporter, SpanExporter};
use tower_http::{
    classify::{ServerErrorsAsFailures, SharedClassifier},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, MakeSpan, OnResponse, TraceLayer},
};

use std::{
    env,
    sync::{Arc, OnceLock},
};
use tracing_subscriber::{fmt, layer::SubscriberExt, registry, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    dotenv().ok();
    let telemetry_guard = init_telemetry(
        &env::var("OTEL_SERVICE_NAME").unwrap().to_string(),
        &env::var("OTEL_EXPORT_OTLP_ENDPOINT").unwrap().to_string(),
    )
    .unwrap();
    // init_tracing();

    let app_state = Arc::new(AppState::init().await.unwrap());

    let app = create_router(app_state)
        .layer(init_req_tracer())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    // telemetry_guard.shutdown();
    tokio::signal::ctrl_c().await.unwrap();

    tracing::warn!("Shutdown signal received!");
}

fn init_tracing() {
    let fmt_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_target(false)
        .with_ansi(false)
        .with_timer(fmt::time::UtcTime::rfc_3339());

    registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tpwer_http=info", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(fmt_layer)
        .init();
}

fn init_req_tracer()
-> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, MakeSpanWithRequestId, DefaultOnRequest> {
    TraceLayer::new_for_http()
        .make_span_with(MakeSpanWithRequestId)
        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO))
}

#[derive(Clone)]
struct MakeSpanWithRequestId;

impl<B> MakeSpan<B> for MakeSpanWithRequestId {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        let req_id = request
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        let method = request.method().as_str();
        let path = request.uri().path();
        tracing::info_span!(
            "HTTP request",
            otel.name = %format!("{} {}", method, path),
            http.method = %method,
            http.route = %path,
            http.target = %request.uri(),
            http.scheme = "http",
            http.response.status_code = tracing::field::Empty,
            otel.status_code = tracing::field::Empty,
            request_id = %req_id
        )
    }
}

#[derive(Clone)]
struct HttpOnResponse;

impl<B> OnResponse<B> for HttpOnResponse {
    fn on_response(
        self,
        response: &axum::http::Response<B>,
        latency: std::time::Duration,
        span: &tracing::Span,
    ) {
        let status = response.status().as_u16();
        span.record("http.response.status.code", status as i64);

        if status >= 500 {
            span.record("otel.status_code", "ERROR");
        } else {
            span.record("otel.status_code", "OK");
        }

        tracing::info!(
            http.response.status_code = status,
            latency_ms = latency.as_secs_f64() * 1000.0,
            "Finished processing request"
        )
    }
}
