use api::{routes::create_router, types::app_state::AppState};
use dotenvy::dotenv;
use tower_http::{
    classify::{ServerErrorsAsFailures, SharedClassifier},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, MakeSpan, TraceLayer},
};

use std::sync::Arc;
use tracing_subscriber::{fmt, layer::SubscriberExt, registry, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    dotenv().ok();
    init_tracing();

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
        tracing::info_span!(
            "request",
            methid = %request.method(),
            path = %request.uri().path(),
            request_id = %req_id
        )
    }
}
