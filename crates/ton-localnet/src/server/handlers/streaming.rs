use crate::localnet::Localnet;
use crate::server::ShutdownSignal;
use crate::streaming::{
    StreamingEnvelope, StreamingErrorResponse, StreamingOperation, StreamingStatusResponse,
    StreamingSubscribeRequest, StreamingSubscription, StreamingUnsubscribeRequest,
    notifications_for_commit, validate_unsubscribe_request,
};
use axum::{
    Json,
    body::Bytes,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use serde::de::DeserializeOwned;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;

pub async fn streaming_sse(
    State(node): State<Arc<Localnet>>,
    State(shutdown): State<ShutdownSignal>,
    body: Bytes,
) -> Response {
    let payload = match serde_json::from_slice::<StreamingSubscribeRequest>(&body) {
        Ok(payload) => payload,
        Err(e) => return streaming_bad_request(None, format!("invalid subscription request: {e}")),
    };
    let subscription = match StreamingSubscription::from_subscribe_request(&payload) {
        Ok(subscription) => subscription,
        Err(e) => return streaming_bad_request(payload.id, e.to_string()),
    };

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);
    let _ = tx
        .send(Ok(sse_json_event(
            "connected",
            &StreamingStatusResponse {
                id: payload.id,
                status: "subscribed",
            },
        )))
        .await;

    tokio::spawn(stream_sse_notifications(
        node,
        subscription,
        tx,
        shutdown.subscribe(),
    ));

    Sse::new(ReceiverStream::new(rx))
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        )
        .into_response()
}

pub async fn streaming_ws(
    State(node): State<Arc<Localnet>>,
    State(shutdown): State<ShutdownSignal>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, node, shutdown.subscribe()))
}

async fn stream_sse_notifications(
    node: Arc<Localnet>,
    subscription: StreamingSubscription,
    tx: mpsc::Sender<Result<Event, Infallible>>,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut commits = node.subscribe_streaming_events();

    loop {
        let commit = tokio::select! {
            _ = shutdown.recv() => break,
            commit = commits.recv() => match commit {
                Ok(commit) => commit,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
        };

        let notifications = match notifications_for_commit(&node, &subscription, commit).await {
            Ok(notifications) => notifications,
            Err(e) => {
                tracing::debug!("Failed to build streaming notification: {e:?}");
                continue;
            }
        };

        for notification in notifications {
            if tx
                .send(Ok(sse_json_event("event", &notification)))
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

async fn handle_ws(
    mut socket: WebSocket,
    node: Arc<Localnet>,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut subscription = StreamingSubscription::default();
    let mut commits = node.subscribe_streaming_events();

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                break;
            }
            message = socket.recv() => {
                let Some(message) = message else {
                    break;
                };
                let message = match message {
                    Ok(message) => message,
                    Err(e) => {
                        tracing::debug!("Streaming websocket read failed: {e:?}");
                        break;
                    }
                };

                if !handle_ws_message(&mut socket, &mut subscription, &message).await {
                    break;
                }
            }
            commit = commits.recv(), if !subscription.event_types.is_empty() => {
                let commit = match commit {
                    Ok(commit) => commit,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                let notifications = match notifications_for_commit(&node, &subscription, commit).await {
                    Ok(notifications) => notifications,
                    Err(e) => {
                        tracing::debug!("Failed to build streaming websocket notification: {e:?}");
                        continue;
                    }
                };

                for notification in notifications {
                    if send_ws_json(&mut socket, &notification).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

async fn handle_ws_message(
    socket: &mut WebSocket,
    subscription: &mut StreamingSubscription,
    message: &Message,
) -> bool {
    match message {
        Message::Text(_) | Message::Binary(_) => {}
        Message::Ping(payload) => {
            let _ = socket.send(Message::Pong(payload.clone())).await;
            return true;
        }
        Message::Close(_) => return false,
        Message::Pong(_) => return true,
    }

    let envelope = match parse_ws_json::<StreamingEnvelope>(message) {
        Ok(envelope) => envelope,
        Err(e) => {
            let _ = send_ws_error(socket, None, format!("invalid request envelope: {e}")).await;
            return true;
        }
    };

    match envelope.operation {
        StreamingOperation::Ping => {
            let _ = send_ws_json(
                socket,
                &StreamingStatusResponse {
                    id: envelope.id,
                    status: "pong",
                },
            )
            .await;
        }
        StreamingOperation::Subscribe => {
            let request = match parse_ws_json::<StreamingSubscribeRequest>(message) {
                Ok(request) => request,
                Err(e) => {
                    let _ = send_ws_error(
                        socket,
                        envelope.id,
                        format!("invalid subscribe request: {e}"),
                    )
                    .await;
                    return true;
                }
            };

            match StreamingSubscription::from_subscribe_request(&request) {
                Ok(next_subscription) => {
                    *subscription = next_subscription;
                    let _ = send_ws_json(
                        socket,
                        &StreamingStatusResponse {
                            id: request.id.or(envelope.id),
                            status: "subscribed",
                        },
                    )
                    .await;
                }
                Err(e) => {
                    let _ = send_ws_error(socket, request.id.or(envelope.id), e.to_string()).await;
                }
            }
        }
        StreamingOperation::Unsubscribe => {
            let request = match parse_ws_json::<StreamingUnsubscribeRequest>(message) {
                Ok(request) => request,
                Err(e) => {
                    let _ = send_ws_error(
                        socket,
                        envelope.id,
                        format!("invalid unsubscribe request: {e}"),
                    )
                    .await;
                    return true;
                }
            };

            if let Err(e) = validate_unsubscribe_request(&request)
                .and_then(|()| subscription.unsubscribe(&request))
            {
                let _ = send_ws_error(socket, request.id.or(envelope.id), e.to_string()).await;
                return true;
            }

            let _ = send_ws_json(
                socket,
                &StreamingStatusResponse {
                    id: request.id.or(envelope.id),
                    status: "unsubscribed",
                },
            )
            .await;
        }
    }

    true
}

fn parse_ws_json<T: DeserializeOwned>(message: &Message) -> anyhow::Result<T> {
    match message {
        Message::Text(text) => Ok(serde_json::from_str(text)?),
        Message::Binary(bytes) => Ok(serde_json::from_slice(bytes)?),
        _ => anyhow::bail!("expected text or binary JSON message"),
    }
}

async fn send_ws_error(
    socket: &mut WebSocket,
    id: Option<String>,
    error: String,
) -> Result<(), axum::Error> {
    send_ws_json(socket, &StreamingErrorResponse { id, error }).await
}

async fn send_ws_json<T: serde::Serialize + Sync>(
    socket: &mut WebSocket,
    value: &T,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(value).unwrap_or_else(|e| {
        serde_json::json!({
            "error": format!("failed to serialize websocket response: {e}")
        })
        .to_string()
    });
    socket.send(Message::Text(text.into())).await
}

fn streaming_bad_request(id: Option<String>, error: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(StreamingErrorResponse { id, error }),
    )
        .into_response()
}

fn sse_json_event<T: serde::Serialize>(event: &'static str, value: &T) -> Event {
    let data = serde_json::to_string(value).unwrap_or_else(|e| {
        serde_json::json!({
            "error": format!("failed to serialize event: {e}")
        })
        .to_string()
    });
    Event::default().event(event).data(data)
}
