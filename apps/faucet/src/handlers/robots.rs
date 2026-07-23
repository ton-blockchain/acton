use axum::{http::header::CONTENT_TYPE, response::IntoResponse};

pub(super) async fn robots_txt() -> impl IntoResponse {
    let robots_txt = r#"User-agent: *
Disallow: /
"#;

    ([(CONTENT_TYPE, "text/plain; charset=utf-8")], robots_txt)
}
