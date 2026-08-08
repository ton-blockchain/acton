use axum::{http::header::CONTENT_TYPE, response::IntoResponse};

pub async fn handler() -> impl IntoResponse {
    let robots_txt = "User-agent: *\nDisallow: /\n";

    ([(CONTENT_TYPE, "text/plain; charset=utf-8")], robots_txt)
}
