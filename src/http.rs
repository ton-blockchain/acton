use std::env;
use std::ffi::OsStr;

pub(crate) const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";

pub(crate) fn blocking_client_builder() -> reqwest::blocking::ClientBuilder {
    let builder = reqwest::blocking::Client::builder().user_agent(crate::build_info::user_agent());
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

pub(crate) fn client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder().user_agent(crate::build_info::user_agent());
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

#[cfg(test)]
mod tests {
    use super::{blocking_client_builder, client_builder, proxy_enabled_from_value};
    use std::ffi::OsStr;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn acton_use_proxy_is_disabled_by_default() {
        assert!(!proxy_enabled_from_value(None));
    }

    #[test]
    fn acton_use_proxy_accepts_1_or_true() {
        for value in ["1", "true"] {
            assert!(proxy_enabled_from_value(Some(OsStr::new(value))));
        }
    }

    #[test]
    fn acton_use_proxy_rejects_other_values() {
        for value in ["", "0", "false", "TRUE", "yes"] {
            assert!(!proxy_enabled_from_value(Some(OsStr::new(value))));
        }
    }

    #[test]
    fn blocking_client_uses_acton_user_agent() {
        let (url, request) = capture_request();
        blocking_client_builder()
            .build()
            .unwrap()
            .get(url)
            .send()
            .unwrap();

        assert_acton_user_agent(&request.join().unwrap());
    }

    #[tokio::test]
    async fn async_client_uses_acton_user_agent() {
        let (url, request) = capture_request();
        client_builder()
            .build()
            .unwrap()
            .get(url)
            .send()
            .await
            .unwrap();

        assert_acton_user_agent(&request.join().unwrap());
    }

    fn capture_request() -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let request = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let bytes_read = stream.read(&mut buffer).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
            String::from_utf8_lossy(&buffer[..bytes_read]).into_owned()
        });
        (format!("http://{address}"), request)
    }

    fn assert_acton_user_agent(request: &str) {
        let expected = crate::build_info::user_agent();
        assert!(request.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("user-agent") && value.trim() == expected
            })
        }));
    }
}
