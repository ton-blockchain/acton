# toncenter-client

An asynchronous Rust client for TON Center v2 and v3. Query account state, run get
methods, read indexed transactions, and broadcast signed messages with typed requests
and responses from the `toncenter` crate.

## Installation

```toml
[dependencies]
toncenter-client = "2.1.15"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Calls require a Tokio runtime with I/O and timers enabled. Keep one client for each
configuration and clone it when sharing it with tasks. Clones share connections,
concurrency limits, and quota state.

## Account balance and indexed transactions

```rust,no_run
use toncenter_client::{Client, toncenter::{v2, v3}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .mainnet()
        .api_key(std::env::var("TONCENTER_API_KEY").ok())
        .user_agent("my-app/1.0")
        .build()?;
    let address = "0:0000000000000000000000000000000000000000000000000000000000000000";

    let balance = client.v2().get_address_balance(&v2::requests::AddressBalanceRequest {
        address: address.to_owned(),
        seqno: None,
    }).await?;
    println!("Balance: {balance} nanograms");

    let transactions = client.v3().get_transactions(&v3::requests::TransactionsQuery {
        account: vec![address.to_owned()],
        limit: Some(10),
        ..Default::default()
    }).await?;
    println!("Transactions: {}", transactions.transactions.len());
    Ok(())
}
```

V2 methods return the `result` inside the API envelope. V3 methods return the direct
JSON response. Monetary strings preserve the server's full integer precision; one
GRAM is 1,000,000,000 nanograms. Request and response field documentation describes
address formats, units, pagination, and optional values.

## Endpoints and authentication

Use `mainnet()` or `testnet()` for the public service. For other deployments, set
`v2_url` and `v3_url` to their complete base URLs, including API path prefixes. The
versions can use different hosts. A client can also configure only one version.
Trailing slashes are optional; prefixes such as `/gateway/api/v2` are preserved.

```rust,no_run
use toncenter_client::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .v2_url("https://rpc.example.com/gateway/api/v2")
        .v3_url("https://indexer.example.com/api/v3")
        .api_key(std::env::var("TONCENTER_API_KEY").ok())
        .origin("https://app.example.com")
        .user_agent("my-app/1.0")
        .build()?;
    println!("API key configured: {}", client.has_api_key());
    Ok(())
}
```

`api_key(Some(key))` sends `X-API-Key`; `api_key(None)` removes it. `origin` and
`user_agent` apply to both API versions and every retry. User-Agent defaults to
`toncenter-client/<version>` and is replaced in full when set. Origin is absent
unless configured. Use `bearer_auth` when a private gateway also requires an
Authorization header. The library does not read API-key environment variables;
the examples read them in the application.

Redirects are disabled. API URLs must use HTTP or HTTPS and contain no user info,
query, or fragment. System proxy settings are enabled by default; use
`system_proxy(false)` to connect directly.

## Retry and timeout configuration

```rust,no_run
use std::time::Duration;
use toncenter_client::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .testnet()
        .max_attempts(5)
        .retry_delays(Duration::from_millis(250), Duration::from_secs(4))
        .connect_timeout(Duration::from_secs(5))
        .request_timeout(Duration::from_secs(15))
        .operation_timeout(Duration::from_secs(60))
        .build()?;
    println!("{client:?}");
    Ok(())
}
```

| Setting | Default | Effect |
| --- | --- | --- |
| `max_attempts` | 3 | Total attempts, including the first. Set 1 to disable retries. |
| `retry_delays` | 500 ms, 5 s | Initial and maximum exponential delay, before jitter. |
| `connect_timeout` | 10 s | Connection establishment for one attempt. |
| `request_timeout` | 30 s | One HTTP attempt, including the response body. |
| `operation_timeout` | 90 s | Complete call, including concurrency waits, quota waits, and retries. |
| `max_concurrent_requests` | 16 | Active calls across client clones. |
| `max_response_bytes` | 16 MiB | Maximum response body size before decoding. |

Backoff doubles after each failed attempt, up to the configured maximum. Each delay
is randomly selected from 50% to 100% of that value. `Retry-After`, as seconds or an
HTTP date, takes precedence. The operation deadline can expire while waiting for it.

Read operations retry connection and response-transfer failures, timeouts, and HTTP
408, 429, 500, 502, 503, and 504 responses. Structured API errors retry codes 408,
429, 502, 503, and 504, including when HTTP status is 200. API code 500 can indicate
a deterministic contract or `TONLib` failure and is not retried automatically.
Malformed JSON, response type mismatches, and oversized responses are not retried.

## Broadcast delivery

`send_boc`, `send_boc_return_hash`, and v3 `send_message` receive one attempt by
default. A lost response can occur after the server accepts the message. Use
`retry_broadcasts(true)` only when the application accepts that uncertainty and
resending an identical message. Retries reuse the exact serialized body; the
client does not sign messages, update wallet seqnos, or extend expiration times.

An accepted broadcast does not confirm on-chain execution. Query transactions or
traces to determine its outcome. Cancelling a future stops waiting and further
retries, but cannot retract a message that reached the server.

## Request quotas

Anonymous public endpoints use a 1100 ms interval between request starts.
Authenticated and custom endpoints have no default interval. Use `request_interval`
for your subscription or server quota; zero disables normal pacing.

V2, v3, and independent clients using the same origin and API key share a quota
within the process. Mainnet and testnet have separate origins and quotas. Use the
same non-secret `quota_group` name when different hosts or keys consume one quota.
Shared groups keep the greatest configured interval for the lifetime of the group.
Idle time does not accumulate permits. Every retry also consumes a quota slot.

A 429 response delays the whole group by `Retry-After`, or by the calculated backoff
with a minimum of 1100 ms. This cooldown also applies when the failing call has no
attempts left. A cancelled quota waiter consumes no slot. Quotas are process-local;
coordinate multiple application processes separately when they share a server quota.

## Generic calls and JSON-RPC

Named v2 methods use GET where supported and POST otherwise. Select another
transport through the borrowed v2 interface:

```rust,no_run
use toncenter_client::{Client, V2Transport, toncenter::v2};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder().testnet().build()?;
    let request = v2::requests::EmptyRequest {};
    let info = client.v2().transport(V2Transport::JsonRpc)
        .get_masterchain_info(&request).await?;
    println!("Masterchain block: {}", info.last.seqno);

    let envelope = client.v2_response::<v2::responses::MasterchainInfo>(
        V2Transport::Get, "getMasterchainInfo", &request,
    ).await?;
    println!("Response metadata: {}", envelope.extra);
    Ok(())
}
```

`call_v2::<Endpoint>` and `call_v3::<Endpoint>` accept the typed endpoint markers
from `toncenter`. `v2_request::<Response>`, `v3_get::<Response>`, and
`v3_post::<Response>` accept application-selected response types. Use these for
server extensions or partial responses. Method names and paths are relative to
the configured API base. GET arrays become repeated query parameters; absent
optional values are omitted. JSON bodies retain their request type's serialization.

Unknown v2 operations and unknown v3 POST routes receive one attempt. For supported
broadcast routes, only `retry_broadcasts` enables repeated delivery.

## Errors and diagnostics

A failed call returns `Error` after applying the retry policy. `kind()` classifies
the failure; `status()` is the last received HTTP status, if available. `attempts()`
counts started attempts and is zero when the call expires waiting for its first
quota slot. `operation()` identifies the method without request parameters.

`api_error()` exposes the server's code and diagnostic text. `detail()` provides a
configuration explanation or a JSON field path. Display and Debug omit credentials
and response bodies. Server diagnostic text can contain echoed input; select what
your application displays or records. Transport errors retain their source without
the request URL.

The client emits `tracing` debug events for attempts, retries, and completion, with
operation, target, duration, and outcome. Configure a tracing subscriber in the
application to collect them.

## Tests

`cargo test -p toncenter-client` runs transport tests against local HTTP servers.
Public-network checks are opt-in:

```sh
cargo test -p toncenter-client --features live-tests --test live
```

Set `TONCENTER_API_KEY` for those live checks when a key is available.
