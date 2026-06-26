use std::sync::LazyLock;

use opentelemetry::{
    global,
    metrics::{Counter, Histogram, Meter},
};

pub static METER: LazyLock<Meter> = LazyLock::new(|| global::meter("rust-axum-app"));

pub static HTTP_REQUESTS_TOTAL: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("http.requests.total")
        .with_description("Total Number of HTTP Requests")
        .with_unit("{request}")
        .build()
});

pub static HTTP_REQUEST_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    METER
        .f64_histogram("http.request.duration")
        .with_description("HTTP Request durations in ms")
        .with_unit("ms")
        .with_boundaries(vec![
            1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0,
        ])
        .build()
});

pub static CACHE_LOOKUPS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("cache.lookups.total")
        .with_description("The Number of Cache Lookups")
        .with_unit("request")
        .build()
});

pub static ARTIFACTS_CREATED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    METER
        .u64_counter("artifacts.created")
        .with_description("Total artifacts cached")
        .with_unit("request")
        .build()
});
