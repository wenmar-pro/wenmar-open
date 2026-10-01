//! Serving connections, with limits on what a client may send before the
//! application sees a request.
//!
//! `axum::serve` has no settings for either limit, and its defaults let one
//! request head be 400 KB and let a connection that never finishes its head
//! stay open for ever. The application's own limits cannot help: by the time
//! a layer runs, the head is already in memory.

use std::future::Future;
use std::io::ErrorKind;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ConnectInfo;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::{Semaphore, mpsc, watch};
use tower::ServiceExt;

/// Largest request head read: the request line and every header together.
/// A larger one is answered 431 and the connection is closed. The API's own
/// requests are under 1 KB, and browsers and proxies add a few more.
pub const HEAD_LIMIT: usize = 32 * 1024;

/// Longest a connection may take to send a request head, counted from when
/// the server starts waiting for one. That includes the wait between two
/// requests on a connection that is kept open, so it is longer than the 90
/// seconds a reverse proxy usually keeps an idle connection: the proxy
/// closes first, and never sends a request into a connection this side is
/// closing.
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(120);

/// Most connections open at once. Each may hold a head of [`HEAD_LIMIT`]
/// before the application can refuse anything, so this bounds that memory:
/// 1,024 heads of 32 KB are 32 MB. Further connections wait, unaccepted, in
/// the operating system's queue, where they cost this process nothing.
pub const MOST_CONNECTIONS: usize = 1024;

/// The limits on connections. `Limits::default()` is what the server runs
/// with; tests make them small.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// See [`HEAD_TIMEOUT`].
    pub head_timeout: Duration,
    /// See [`MOST_CONNECTIONS`].
    pub connections: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            head_timeout: HEAD_TIMEOUT,
            connections: MOST_CONNECTIONS,
        }
    }
}

/// Serves `app` on `listener`, within `limits`, until `shutdown` finishes.
/// Then it stops accepting, lets requests in flight finish, closes idle
/// connections and returns.
///
/// Each request carries the peer's address as [`ConnectInfo`].
pub async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()>,
    limits: Limits,
) {
    let mut http = http1::Builder::new();
    http.timer(TokioTimer::new())
        .max_buf_size(HEAD_LIMIT)
        .header_read_timeout(limits.head_timeout);

    // `stop` tells every connection to finish. `finished` is held by every
    // connection, so the channel closes when the last one is done.
    let (stop, stopping) = watch::channel(false);
    let (finished, mut all_finished) = mpsc::channel::<()>(1);
    let places = Arc::new(Semaphore::new(limits.connections.max(1)));
    let mut shutdown = pin!(shutdown);
    loop {
        // A place first, then a connection: one that has no place yet stays
        // in the operating system's queue.
        let place = tokio::select! {
            place = Arc::clone(&places).acquire_owned() => place,
            () = &mut shutdown => break,
        };
        let Ok(place) = place else { break };
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = &mut shutdown => break,
        };
        let (stream, peer) = match accepted {
            Ok(accepted) => accepted,
            // The client went away before it was accepted.
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::ConnectionRefused
                        | ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionReset
                ) =>
            {
                continue;
            }
            // Out of file descriptors, most likely. Waiting lets some close.
            Err(error) => {
                tracing::warn!(%error, "cannot accept a connection");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        let app = app.clone();
        let service = service_fn(move |mut request: hyper::Request<Incoming>| {
            request.extensions_mut().insert(ConnectInfo(peer));
            app.clone().oneshot(request)
        });
        let connection = http.serve_connection(TokioIo::new(stream), service);
        let mut stopping = stopping.clone();
        let finished = finished.clone();
        tokio::spawn(async move {
            let mut connection = pin!(connection);
            // A connection that fails is the client's doing: it sent
            // something that is not HTTP, was too slow, or went away.
            tokio::select! {
                _ = connection.as_mut() => {}
                _ = stopping.changed() => {
                    connection.as_mut().graceful_shutdown();
                    let _ = connection.await;
                }
            }
            drop(place);
            drop(finished);
        });
    }
    drop(listener);
    let _ = stop.send(true);
    drop(finished);
    // `None` once every connection has dropped its sender.
    let _ = all_finished.recv().await;
}
