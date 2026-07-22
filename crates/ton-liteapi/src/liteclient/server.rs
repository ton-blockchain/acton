use std::future::poll_fn;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;

use crate::adnl::AdnlPeer;
use crate::adnl::crypto::KeyPair;
use tokio::net::TcpListener;
use tokio::net::ToSocketAddrs;
use tokio_tower::multiplex::Server;
use tower::MakeService;
use tower::Service;

use crate::liteclient::peer::LitePeer;
use crate::tl::adnl::Message;

pub async fn serve<A, M>(
    addr: &A,
    private_key: KeyPair,
    mut service_maker: M,
) -> Result<(), Box<dyn std::error::Error>>
where
    A: ToSocketAddrs,
    M: MakeService<SocketAddr, Message, Response = Message> + Send,
    M::Error: std::fmt::Debug,
    M::MakeError: std::error::Error,
    M::Service: Send + 'static,
    <M::Service as Service<Message>>::Future: Send,
{
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (socket, addr) = match listener.accept().await {
            Ok((socket, addr)) => (socket, addr),
            Err(e) => {
                // [From `hyper::Server` in 0.14](https://github.com/hyperium/hyper/blob/v0.14.27/src/server/tcp.rs#L186)
                //
                // > A possible scenario is that the process has hit the max open files
                // > allowed, and so trying to accept a new connection will fail with
                // > `EMFILE`. In some cases, it's preferable to just wait for some time, if
                // > the application will likely close some files (or connections), and try
                // > to accept the connection again. If this option is `true`, the error
                // > will be logged at the `error` level, since it is still a big deal,
                // > and then the listener will sleep for 1 second.
                if !matches!(
                    e.kind(),
                    ErrorKind::ConnectionRefused
                        | ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionReset
                ) {
                    log::error!("accept error: {e}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
        };
        log::debug!("[{addr:?}] Accepted socket");
        if let Err(e) = poll_fn(|cx| service_maker.poll_ready(cx)).await {
            log::error!("[{addr:?}] Polling failed: {:?}", e);
            continue;
        };
        let service = match service_maker.make_service(addr).await {
            Ok(x) => x,
            Err(e) => {
                log::error!("[{addr:?}] Making service failed: {:?}", e);
                continue;
            }
        };
        tokio::spawn(async move {
            let adnl = match AdnlPeer::handle_handshake(socket, |_| Some(private_key)).await {
                Ok(x) => x,
                Err(e) => {
                    log::error!("[{addr:?}] Handshake failed: {:?}", e);
                    return;
                }
            };
            log::debug!("[{addr:?}] Handshake performed");
            let lite = LitePeer::new(adnl);
            if let Err(e) = Server::new(lite, service).await {
                log::error!("[{addr:?}] Server failed: {:?}", e);
            }
        });
    }
}
