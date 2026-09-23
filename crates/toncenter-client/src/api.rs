use serde::{Serialize, de::DeserializeOwned};
use toncenter::{v2, v3};

use crate::{Client, Error, V2Transport};

/// Borrowed v2 interface with named, typed operations.
///
/// Each result is unwrapped from the v2 response envelope. Request values retain
/// the API's wire types; use the request field documentation for units and formats.
#[derive(Clone, Copy, Debug)]
pub struct V2<'a> {
    pub(crate) client: &'a Client,
    pub(crate) transport: Option<V2Transport>,
}

impl V2<'_> {
    /// Selects a transport for calls through this view. By default, endpoints
    /// supporting GET use query parameters; remaining endpoints use REST POST.
    ///
    /// JSON-RPC sends all calls to `jsonRPC` with the same authentication and retry policy.
    #[must_use]
    pub const fn transport(mut self, transport: V2Transport) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Calls an endpoint marker, preserving its request and response type pairing.
    /// Errors use the same categories and attempt accounting as named calls.
    pub async fn call<E: v2::endpoints::Endpoint>(
        &self,
        request: &E::Request,
    ) -> Result<E::Response, Error>
    where
        E::Request: Sync,
    {
        let transport = self.transport.unwrap_or(if E::SUPPORTS_GET {
            V2Transport::Get
        } else {
            V2Transport::Post
        });
        self.client.v2_request(transport, E::METHOD, request).await
    }

    /// Calls a v2 method with application-selected response types, including stack
    /// extensions. The default for this generic form is REST POST.
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        request: &(impl Serialize + Sync + ?Sized),
    ) -> Result<T, Error> {
        self.client
            .v2_request(self.transport.unwrap_or(V2Transport::Post), method, request)
            .await
    }
}

/// Borrowed v3 interface with named operations and the corresponding typed results.
/// V3 results are direct JSON bodies, without a v2 envelope.
#[derive(Clone, Copy, Debug)]
pub struct V3<'a> {
    pub(crate) client: &'a Client,
}

macro_rules! v2_methods {
    ($($endpoint:ident, $name:ident, $wire:tt, $request:ty, $response:ty, $get:literal, $description:literal;)+) => {
        impl V2<'_> {
            $(
                #[doc = $description]
                #[doc = "\n\nSee the request type for parameter defaults and formats. API and transport failures return [`Error`]."]
                pub async fn $name(&self, request: &$request) -> Result<$response, Error> {
                    self.call::<v2::endpoints::$endpoint>(request).await
                }
            )+
        }
    };
}

macro_rules! v3_methods {
    ($($endpoint:ident, $name:ident, $method:ident, $wire:tt, $request:ty, $response:ty, $description:literal;)+) => {
        impl V3<'_> {
            $(
                #[doc = $description]
                #[doc = "\n\nSee the request type for parameter defaults and formats. API and transport failures return [`Error`]."]
                pub async fn $name(&self, request: &$request) -> Result<$response, Error> {
                    self.client.call_v3::<v3::endpoints::$endpoint>(request).await
                }
            )+
        }
    };
}

toncenter::for_each_v2_endpoint!(v2_methods);
toncenter::for_each_v3_endpoint!(v3_methods);
