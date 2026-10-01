//! The server on a real socket: what it does before a request reaches the
//! application, and how it stops.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::extract::ConnectInfo;
use axum::routing::get;
use open_server::serve::{HEAD_LIMIT, Limits, serve};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::common;

/// A server on a free local port. Dropping `stop` shuts it down.
struct Served {
    address: SocketAddr,
    stop: oneshot::Sender<()>,
    done: JoinHandle<()>,
}

async fn start(app: Router, limits: Limits) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let done = tokio::spawn(serve(
        listener,
        app,
        async move {
            let _ = stopped.await;
        },
        limits,
    ));
    Served {
        address,
        stop,
        done,
    }
}

/// Reads what has arrived, waiting for some. `None` when the peer has closed
/// the connection or reset it.
async fn read_some(stream: &TcpStream) -> Option<Vec<u8>> {
    let mut buffer = [0u8; 4096];
    loop {
        stream.readable().await.ok()?;
        match stream.try_read(&mut buffer) {
            Ok(0) => return None,
            Ok(read) => return Some(buffer[..read].to_vec()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return None,
        }
    }
}

/// Writes all of `bytes`, or as much as the peer takes before it closes.
async fn write_all(stream: &TcpStream, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        if stream.writable().await.is_err() {
            return;
        }
        match stream.try_write(bytes) {
            Ok(written) => bytes = &bytes[written..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return,
        }
    }
}

/// Reads until the peer closes the connection.
async fn read_to_end(stream: &TcpStream) -> String {
    let mut bytes = Vec::new();
    while let Some(more) = read_some(stream).await {
        bytes.extend_from_slice(&more);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Sends one request that asks for the connection to be closed.
async fn exchange(address: SocketAddr, request: &str) -> String {
    let stream = TcpStream::connect(address).await.unwrap();
    // The server may answer and close before the whole request is written.
    write_all(&stream, request.as_bytes()).await;
    tokio::time::timeout(Duration::from_secs(10), read_to_end(&stream))
        .await
        .expect("the server neither answered nor closed the connection")
}

/// Reads one response with a `content-length` from a connection kept open.
async fn read_response(stream: &TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .map_or(0, |value| value.trim().parse().unwrap());
            if body.len() >= length {
                return text;
            }
        }
        let more = read_some(stream)
            .await
            .expect("the connection closed in the middle of a response");
        bytes.extend_from_slice(&more);
    }
}

#[tokio::test]
async fn the_application_is_served_and_sees_the_peer_address() {
    let app = Router::new().route(
        "/who",
        get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.ip().to_string() }),
    );
    let served = start(app, Limits::default()).await;
    let answer = exchange(
        served.address,
        "GET /who HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n",
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.ends_with("127.0.0.1"), "{answer}");
}

#[tokio::test]
async fn a_request_head_over_the_limit_is_refused() {
    let app = common::app().await;
    let served = start(app.router.clone(), Limits::default()).await;

    // Within the limit: answered.
    let padding = "a".repeat(HEAD_LIMIT / 2);
    let answer = exchange(
        served.address,
        &format!(
            "GET /health HTTP/1.1\r\nhost: x\r\nx-padding: {padding}\r\nconnection: close\r\n\r\n"
        ),
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 200"), "{:.80}", answer);

    // 360 KB of headers, as the final review sent.
    let padding = "a".repeat(360 * 1024);
    let answer = exchange(
        served.address,
        &format!(
            "GET /health HTTP/1.1\r\nhost: x\r\nx-padding: {padding}\r\nconnection: close\r\n\r\n"
        ),
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 431"), "{:.80}", answer);

    // A 60 KB address in the first line.
    let long = "A".repeat(60_000);
    let answer = exchange(
        served.address,
        &format!("POST /v1/vin/batch?x={long} HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n"),
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 431"), "{:.80}", answer);

    // An address over the application's limit but inside the head limit is
    // answered by the application, in its error shape.
    let long = "A".repeat(open_server::MOST_URI + 1);
    let answer = exchange(
        served.address,
        &format!("GET /v1/meta?x={long} HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n"),
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 414"), "{:.80}", answer);
    assert!(answer.contains("\"uri_too_long\""), "{answer}");

    // A body of the size the API accepts still arrives whole.
    let vins = vec!["\"KM8K2CAB4PU001140\""; 50].join(",");
    let body = format!("{{\"vins\":[{vins}]}}");
    let answer = exchange(
        served.address,
        &format!(
            "POST /v1/vin/batch HTTP/1.1\r\nhost: x\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 200"), "{:.80}", answer);
    assert_eq!(answer.matches("\"vin\":\"KM8K2CAB4PU001140\"").count(), 50);
}

#[tokio::test]
async fn a_connection_that_never_finishes_its_head_is_closed() {
    let app = common::app().await;
    let served = start(
        app.router.clone(),
        Limits {
            head_timeout: Duration::from_millis(300),
            ..Limits::default()
        },
    )
    .await;
    let stream = TcpStream::connect(served.address).await.unwrap();
    write_all(&stream, b"GET /health HTTP/1.1\r\nhost: x\r\nx-never-").await;
    let answer = tokio::time::timeout(Duration::from_secs(5), read_to_end(&stream))
        .await
        .expect("the connection was left open");
    assert!(!answer.starts_with("HTTP/1.1 200"), "{answer}");

    // A connection that sends nothing at all is closed as well.
    let silent = TcpStream::connect(served.address).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), read_to_end(&silent))
        .await
        .expect("the silent connection was left open");

    // One that sends its head in time is answered.
    let answer = exchange(
        served.address,
        "GET /health HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n",
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
}

#[tokio::test]
async fn stopping_finishes_the_request_in_flight_and_closes_idle_connections() {
    let app = common::app().await;
    let served = start(app.router.clone(), Limits::default()).await;

    // An idle connection that has been used once and is kept open.
    let idle = TcpStream::connect(served.address).await.unwrap();
    write_all(&idle, b"GET /health HTTP/1.1\r\nhost: x\r\n\r\n").await;
    assert!(read_response(&idle).await.starts_with("HTTP/1.1 200"));

    // A request that is waiting for the data file when the server stops.
    let hold = common::Hold::default();
    let mut busy = Vec::new();
    for _ in 0..2 {
        let state = app.state.clone();
        let hold = hold.clone();
        busy.push(tokio::spawn(async move {
            state.db().run(move |_| hold.wait()).await.unwrap();
        }));
    }
    hold.until_started(2).await;
    let in_flight = TcpStream::connect(served.address).await.unwrap();
    write_all(
        &in_flight,
        b"GET /v1/vin/KM8K2CAB4PU001140 HTTP/1.1\r\nhost: x\r\n\r\n",
    )
    .await;
    // The request has reached the application and waits for the data file.
    common::until("the request in flight", || {
        app.state.places().available_permits() < open_server::MOST_IN_FLIGHT
    })
    .await;

    drop(served.stop);
    // The idle connection is closed; the server waits for the other.
    let rest = tokio::time::timeout(Duration::from_secs(5), read_to_end(&idle))
        .await
        .expect("the idle connection was left open");
    assert_eq!(rest, "");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !served.done.is_finished(),
        "stopped with a request in flight"
    );
    // Nothing new is accepted.
    assert!(
        TcpStream::connect(served.address).await.is_err(),
        "still accepting connections"
    );

    hold.release();
    let answer = tokio::time::timeout(Duration::from_secs(5), read_to_end(&in_flight))
        .await
        .expect("the request in flight was never answered");
    assert!(answer.starts_with("HTTP/1.1 200"), "{:.80}", answer);
    assert!(answer.contains("\"vin\":\"KM8K2CAB4PU001140\""), "{answer}");
    tokio::time::timeout(Duration::from_secs(5), served.done)
        .await
        .expect("the server did not stop")
        .unwrap();
    for task in busy {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn connections_over_the_limit_wait_until_one_closes() {
    let app = common::app().await;
    let served = start(
        app.router.clone(),
        Limits {
            connections: 2,
            ..Limits::default()
        },
    )
    .await;
    // Two connections take both places. Each has been answered once, so
    // the server is known to hold it, and is then kept open with nothing
    // more sent.
    let first = TcpStream::connect(served.address).await.unwrap();
    let second = TcpStream::connect(served.address).await.unwrap();
    for held in [&first, &second] {
        write_all(held, b"GET /health HTTP/1.1\r\nhost: x\r\n\r\n").await;
        assert!(read_response(held).await.starts_with("HTTP/1.1 200"));
    }

    // A third is not served yet, however complete its request.
    let third = TcpStream::connect(served.address).await.unwrap();
    write_all(
        &third,
        b"GET /health HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n",
    )
    .await;
    let early = tokio::time::timeout(Duration::from_millis(300), read_some(&third)).await;
    assert!(early.is_err(), "a third connection was served at once");

    // When one of the two closes, it is.
    drop(first);
    let answer = tokio::time::timeout(Duration::from_secs(5), read_to_end(&third))
        .await
        .expect("the third connection was never served");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    drop(second);
}
