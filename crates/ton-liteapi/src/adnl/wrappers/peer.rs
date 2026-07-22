use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::adnl::crypto::{KeyPair, PublicKey};
use crate::adnl::helper_types::AdnlConnectionInfo;
use crate::adnl::{AdnlAddress, AdnlBuilder, AdnlError, AdnlHandshake};
use futures::{Sink, SinkExt, Stream, StreamExt};
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::bytes::Bytes;
use tokio_util::codec::{Decoder, Framed};

use crate::adnl::primitives::codec::AdnlCodec;

/// Abstraction over [`AdnlSender`] and [`AdnlReceiver`] to keep things simple
#[pin_project]
pub struct AdnlPeer<T>
where
    T: AsyncRead + AsyncWrite,
{
    #[pin]
    stream: Framed<T, AdnlCodec>,
    connection_info: AdnlConnectionInfo,
}

impl AdnlPeer<TcpStream> {
    /// Connect ADNL client to specified server over [`TcpStream`]
    pub async fn connect<A: ToSocketAddrs>(
        server_public: impl AsRef<[u8]>,
        server_address: A,
    ) -> Result<AdnlPeer<TcpStream>, AdnlError> {
        log::debug!("Connecting to TCP server");
        let transport = TcpStream::connect(server_address).await?;
        log::debug!("Connected to {:?}, handshaking...", transport.peer_addr());
        let client = Self::perform_handshake(transport, server_public).await?;
        log::debug!(
            "Handshake completed, connected to {:?}",
            client.connection_info.remote_address()
        );
        Ok(client)
    }

    /// Connect ADNL client to specified server over [`TcpStream`] with a bounded TCP connect and handshake wait.
    pub async fn connect_with_timeout<A: ToSocketAddrs>(
        server_public: impl AsRef<[u8]>,
        server_address: A,
        timeout: Duration,
    ) -> Result<AdnlPeer<TcpStream>, AdnlError> {
        log::debug!("Connecting to TCP server");
        let transport = tokio::time::timeout(timeout, TcpStream::connect(server_address))
            .await
            .map_err(|_| AdnlError::Timeout {
                operation: "tcp_connect",
                timeout,
            })??;
        log::debug!("Connected to {:?}, handshaking...", transport.peer_addr());
        let client =
            tokio::time::timeout(timeout, Self::perform_handshake(transport, server_public))
                .await
                .map_err(|_| AdnlError::Timeout {
                    operation: "adnl_handshake",
                    timeout,
                })??;
        log::debug!(
            "Handshake completed, connected to {:?}",
            client.connection_info.remote_address()
        );
        Ok(client)
    }
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin> AdnlPeer<T> {
    /// Act as a client: perform handshake built from random client keys and `remote_public` over `transport` and check that handshake was successful.
    /// That is a simple version of `perform_custom_handshake`, which uses random protocol parameters.
    /// Returns client part of ADNL connection
    pub async fn perform_handshake(
        transport: T,
        remote_public: impl AsRef<[u8]>,
    ) -> Result<Self, AdnlError> {
        let local_keypair = KeyPair::generate(&mut rand::rngs::OsRng);
        let remote_public = remote_public
            .as_ref()
            .try_into()
            .ok()
            .and_then(PublicKey::from_bytes)
            .ok_or(AdnlError::InvalidPublicKey)?;
        let handshake = AdnlBuilder::with_random_aes_params(&mut rand::rngs::OsRng)
            .perform_ecdh(&local_keypair, &remote_public);
        log::debug!("Performing custom handshake {:?}", handshake);
        Self::perform_custom_handshake(transport, &handshake).await
    }

    /// Act as a client: send `handshake` over `transport` and check that handshake was successful
    /// Returns client part of ADNL connection
    pub async fn perform_custom_handshake(
        mut transport: T,
        handshake: &AdnlHandshake,
    ) -> Result<Self, AdnlError> {
        // send handshake
        transport
            .write_all(&handshake.to_bytes())
            .await
            .map_err(AdnlError::IoError)?;

        let mut stream = handshake.make_client_codec().framed(transport);

        // receive empty message to ensure that server knows our AES keys
        if let Some(x) = stream.next().await {
            x?;
            let connection_info =
                AdnlConnectionInfo::new(handshake.sender().into(), handshake.receiver().clone());
            Ok(Self {
                stream,
                connection_info,
            })
        } else {
            Err(AdnlError::EndOfStream)
        }
    }

    /// Act as a server: receive handshake over transport using [`KeyPair`] provided by `keypair_selector`.
    pub async fn handle_handshake<F: Fn(&AdnlAddress) -> Option<KeyPair>>(
        mut transport: T,
        keypair_selector: F,
    ) -> Result<Self, AdnlError> {
        // receive handshake
        let mut packet = [0u8; 256];
        transport
            .read_exact(&mut packet)
            .await
            .map_err(AdnlError::IoError)?;
        let handshake = AdnlHandshake::decrypt_from_raw(&packet, keypair_selector)?;
        let connection_info =
            AdnlConnectionInfo::new(handshake.receiver().clone(), handshake.sender().into());

        let mut server = Self {
            stream: handshake.make_server_codec().framed(transport),
            connection_info,
        };

        // send empty packet to proof knowledge of AES keys
        server.send(Bytes::new()).await?;

        Ok(server)
    }
}

impl<T> Stream for AdnlPeer<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Item = Result<Bytes, AdnlError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}

impl<T> Sink<Bytes> for AdnlPeer<T>
where
    T: AsyncWrite + AsyncRead,
{
    type Error = AdnlError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        self.project().stream.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_close(cx)
    }
}
