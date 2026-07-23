use std::env;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing() {
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let log_format = env::var("LOG_FORMAT").unwrap_or_else(|_| "text".into());

    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));

    let format = match log_format.to_ascii_lowercase().as_str() {
        "json" => fmt::layer()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true)
            .boxed(),

        _ => fmt::layer().boxed(),
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(format)
        .init();
}
