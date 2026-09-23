use std::future::Future;
use std::sync::{Arc, Mutex, Weak, mpsc};

use anyhow::Context;
use tokio::runtime::{Builder, Handle};
use toncenter_client::{Client, ClientBuilder};

/// Synchronous calls backed by a runtime on a separate thread. Clones and other
/// live adapters share that runtime, which stops after the last adapter is dropped.
/// Construction and destruction are safe inside an existing Tokio runtime.
#[derive(Clone)]
pub(crate) struct BlockingClient {
    client: Client,
    runtime: Arc<RuntimeThread>,
}

struct RuntimeThread {
    handle: Handle,
    _shutdown: tokio::sync::mpsc::UnboundedSender<()>,
}

static RUNTIME: Mutex<Weak<RuntimeThread>> = Mutex::new(Weak::new());

impl BlockingClient {
    pub(crate) fn new(builder: ClientBuilder) -> anyhow::Result<Self> {
        let mut shared = RUNTIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let runtime = match shared.upgrade() {
            Some(runtime) => runtime,
            None => {
                let (ready, receiver) = mpsc::sync_channel(1);
                let (shutdown, mut stopped) = tokio::sync::mpsc::unbounded_channel();
                std::thread::Builder::new()
                    .name("acton-toncenter".to_owned())
                    .spawn(move || {
                        let runtime = Builder::new_current_thread().enable_all().build();
                        match runtime {
                            Ok(runtime) => {
                                let _ = ready.send(Ok(runtime.handle().clone()));
                                runtime.block_on(stopped.recv());
                            }
                            Err(error) => {
                                let _ = ready.send(Err(error));
                            }
                        }
                    })?;
                let runtime = Arc::new(RuntimeThread {
                    handle: receiver
                        .recv()
                        .context("TON Center runtime failed to start")??,
                    _shutdown: shutdown,
                });
                *shared = Arc::downgrade(&runtime);
                runtime
            }
        };
        drop(shared);
        let client = runtime.run(async move { builder.build() })??;
        Ok(Self { client, runtime })
    }

    pub(crate) fn call<F, T>(&self, operation: impl FnOnce(Client) -> F) -> anyhow::Result<T>
    where
        F: Future<Output = Result<T, toncenter_client::Error>> + Send + 'static,
        T: Send + 'static,
    {
        self.runtime
            .run(operation(self.client.clone()))?
            .map_err(Into::into)
    }
}

impl RuntimeThread {
    fn run<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> anyhow::Result<T> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.handle.spawn(async move {
            let _ = sender.send(future.await);
        });
        receiver
            .recv()
            .context("TON Center runtime stopped before completing the call")
    }
}
