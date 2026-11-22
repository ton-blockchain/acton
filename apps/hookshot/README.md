# Hookshot

Rust service scaffold that will receive GitHub webhooks and route events to
other repositories, for example by triggering GitHub Actions workflows.

The stack follows the other applications in `apps`: Axum and Tokio for the HTTP
server, Reqwest with rustls and JSON support for outgoing requests, Serde and TOML
for configuration, tracing for logs, and anyhow for errors.

Webhook handling and routing will be added later.

Run from this directory:

```sh
cp config.toml.example config.toml
just run
```

The server listens on `127.0.0.1:3000` by default. `GET /healthz` returns `ok`.
A shared HTTP client with a 30-second request timeout is available in router state
for future handlers. Ctrl+C and SIGTERM (on Unix) gracefully stop the server.

Configuration is loaded from `config.toml` in the current directory. Set
`HOOKSHOT_CONFIG` to load another file. The example lists the default settings;
omitted sections and fields use these defaults.

```sh
curl http://127.0.0.1:3000/healthz
```

Use `just build` for a release build and `just check` to run formatting,
dependency, Clippy, cargo-deny, and test checks. These checks require `just`,
`cargo-shear`, and `cargo-deny` to be installed.
