use anyhow::{Context, bail};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OnceCell};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_CACHE_ENTRIES: usize = 256;
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

type CacheEntry = Arc<(Instant, OnceCell<Arc<Value>>)>;

#[derive(Default)]
struct MetadataCache {
    entries: HashMap<String, CacheEntry>,
    insertion_order: VecDeque<String>,
}

impl MetadataCache {
    fn get_or_insert(&mut self, uri: &str) -> CacheEntry {
        if let Some(entry) = self.entries.get(uri) {
            if entry.0.elapsed() < CACHE_TTL {
                return Arc::clone(entry);
            }
            self.entries.remove(uri);
            self.insertion_order.retain(|cached_uri| cached_uri != uri);
        }

        while self.entries.len() >= MAX_CACHE_ENTRIES {
            let Some(oldest_uri) = self.insertion_order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest_uri);
        }

        let entry = Arc::new((Instant::now(), OnceCell::new()));
        self.entries.insert(uri.to_owned(), Arc::clone(&entry));
        self.insertion_order.push_back(uri.to_owned());
        entry
    }
}

/// Loads bounded JSON documents over HTTP and deduplicates concurrent requests by URI.
#[derive(Clone)]
pub struct OffchainJsonResolver {
    client: reqwest::Client,
    cache: Arc<Mutex<MetadataCache>>,
}

impl OffchainJsonResolver {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = crate::async_http_client_builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            client,
            cache: Arc::new(Mutex::new(MetadataCache::default())),
        })
    }

    pub async fn get_json(&self, uri: &str) -> anyhow::Result<Arc<Value>> {
        let entry = self.cache.lock().await.get_or_insert(uri);
        entry
            .1
            .get_or_try_init(|| async { self.fetch_json(uri).await.map(Arc::new) })
            .await
            .map(Arc::clone)
    }

    async fn fetch_json(&self, uri: &str) -> anyhow::Result<Value> {
        let url = reqwest::Url::parse(uri).context("Invalid off-chain metadata URI")?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!("Off-chain metadata URI must use HTTP or HTTPS");
        }

        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to fetch off-chain metadata")?
            .error_for_status()
            .context("Off-chain metadata server returned an error")?;

        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            bail!("Off-chain metadata exceeds {MAX_RESPONSE_BYTES} bytes");
        }

        let mut body = Vec::with_capacity(
            response
                .content_length()
                .unwrap_or_default()
                .min(MAX_RESPONSE_BYTES as u64) as usize,
        );
        while let Some(chunk) = response
            .chunk()
            .await
            .context("Failed to read off-chain metadata")?
        {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                bail!("Off-chain metadata exceeds {MAX_RESPONSE_BYTES} bytes");
            }
            body.extend_from_slice(&chunk);
        }

        serde_json::from_slice(&body).context("Off-chain metadata is not valid JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_RESPONSE_BYTES, OffchainJsonResolver};
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[tokio::test]
    async fn successful_responses_are_cached() {
        let (uri, server) = serve_once(br#"{"name":"Example"}"#.to_vec());
        let resolver = OffchainJsonResolver::new().unwrap();

        let first = resolver.get_json(&uri).await.unwrap();
        let request = server.join().unwrap();
        let second = resolver.get_json(&uri).await.unwrap();

        assert_eq!(*first, json!({"name": "Example"}));
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        let expected_user_agent = format!("acton/{}", env!("CARGO_PKG_VERSION"));
        assert!(request.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("user-agent") && value.trim() == expected_user_agent
            })
        }));
    }

    #[tokio::test]
    async fn oversized_responses_are_rejected() {
        let (uri, server) = serve_once(vec![b' '; MAX_RESPONSE_BYTES + 1]);
        let resolver = OffchainJsonResolver::new().unwrap();

        let error = resolver.get_json(&uri).await.unwrap_err();
        let _ = server.join().unwrap();

        assert!(error.to_string().contains("exceeds"));
    }

    fn serve_once(body: Vec<u8>) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let request_len = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len(),
            )
            .unwrap();
            stream.write_all(&body).unwrap();
            String::from_utf8_lossy(&request[..request_len]).into_owned()
        });
        (format!("http://{address}/metadata.json"), server)
    }
}
