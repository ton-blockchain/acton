use crate::liteapi::handler;
use crate::localnet::Localnet;
use base64::Engine;
use std::future::{Ready, ready};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{Context, Poll};
use ton_liteapi::adnl::crypto::{KeyPair, SecretKey};
use ton_liteapi::liteclient::layers::{UnwrapMessagesLayer, WrapErrorLayer};
use ton_liteapi::liteclient::server::serve;
use ton_liteapi::liteclient::types::LiteError;
use ton_liteapi::tl::adnl::Message;
use tower::util::BoxService;
use tower::{Service, ServiceBuilder, service_fn};

/// Connection details for the localnet `LiteServer` endpoint.
///
/// Anton and tonutils-go need both a TCP address and the `LiteServer` public key.
/// The key is stable across localnet starts so external configs can be reused.
pub(crate) struct LiteApiEndpoint {
    pub address: SocketAddr,
    pub public_key_base64: String,
}

/// Starts the localnet `LiteServer` endpoint in a background task.
///
/// The upstream `LiteServer` helper owns its listener, so this function performs a
/// short bind probe first to report "address already in use" before spawning the
/// long-running ADNL server. The spawned task logs transport errors; request
/// errors are returned to clients as `liteServer.error` by `WrapErrorLayer`.
pub(crate) async fn spawn_liteapi_server(
    node: Arc<Localnet>,
    port: u16,
) -> Result<LiteApiEndpoint, io::Error> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let probe = tokio::net::TcpListener::bind(address).await?;
    drop(probe);

    let keypair = localnet_keypair();
    let endpoint = LiteApiEndpoint {
        address,
        public_key_base64: base64::engine::general_purpose::STANDARD
            .encode(keypair.public_key.to_bytes()),
    };

    let service_maker = LiteApiMakeService { node };

    tokio::spawn(async move {
        if let Err(error) = serve(&address, keypair, service_maker).await {
            tracing::error!(?error, "localnet LiteAPI server stopped");
        }
    });

    Ok(endpoint)
}

struct LiteApiMakeService {
    node: Arc<Localnet>,
}

impl Service<SocketAddr> for LiteApiMakeService {
    type Response = BoxService<Message, Message, LiteError>;
    type Error = io::Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: SocketAddr) -> Self::Future {
        ready(Ok(make_liteapi_service(Arc::clone(&self.node))))
    }
}

fn make_liteapi_service(node: Arc<Localnet>) -> BoxService<Message, Message, LiteError> {
    let service = ServiceBuilder::new()
        .layer(UnwrapMessagesLayer)
        .layer(WrapErrorLayer)
        .service(service_fn(move |request| {
            let node = Arc::clone(&node);
            async move { handler::handle(node, request).await }
        }));
    BoxService::new(service)
}

fn localnet_keypair() -> KeyPair {
    KeyPair::from(&SecretKey::from_bytes([
        0x41, 0x63, 0x74, 0x6f, 0x6e, 0x20, 0x6c, 0x6f, 0x63, 0x61, 0x6c, 0x6e, 0x65, 0x74, 0x20,
        0x6c, 0x69, 0x74, 0x65, 0x61, 0x70, 0x69, 0x20, 0x73, 0x65, 0x65, 0x64, 0x20, 0x76, 0x31,
        0x00, 0x00,
    ]))
}
