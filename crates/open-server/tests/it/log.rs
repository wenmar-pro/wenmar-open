//! What the server writes to its log, through the whole application.

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::Request;
use tracing_subscriber::fmt::MakeWriter;

use crate::common;

/// Log output kept in memory.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Captured {
        self.clone()
    }
}

/// A real VIN, as the final review sent it. Its serial number is `004352`.
const VIN: &str = "1HGCM82633A004352";

#[tokio::test]
async fn no_address_puts_a_whole_vin_in_the_log() {
    let app = common::app().await;
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(captured.clone())
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    let paths = [
        format!("/v1/vin/{VIN}"),
        format!("/v1/vin/{VIN}?year=2003"),
        format!("/v1/vehicles/{VIN}"),
        format!("/V1/VIN/{VIN}"),
        format!("//v1/vin/{VIN}"),
        format!("/api/v1/vin/{VIN}"),
        format!("/v1/vins/{VIN}"),
        format!("/{VIN}"),
        format!("/v1/vin%2F{VIN}"),
        format!("/v1/vin/{VIN}/"),
        format!("/v1/meta/{VIN}"),
        format!("/mcp/{VIN}"),
    ];
    for path in &paths {
        app.get(path).await;
    }
    // A method is the caller's to choose as well.
    app.send(
        Request::builder()
            .method(VIN)
            .uri("/")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    // So is the length of the address.
    app.get(&format!("/v1/vin/batch?x={}", "A".repeat(60_000)))
        .await;
    app.get(&format!("/{}", "A/".repeat(30_000))).await;
    drop(guard);

    let text = captured.text();
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("open_server::log"))
        .collect();
    assert_eq!(lines.len(), paths.len() + 3, "{text}");
    assert!(!text.contains("004352"), "{text}");
    assert!(!text.contains("A004"), "{text}");
    // What is kept names the maker and the model, and the route.
    assert!(text.contains("GET /v1/vin/1HGCM82633A "), "{text}");
    assert!(text.contains("GET /v1/vehicles/1HGCM82633A "), "{text}");
    assert!(text.contains("GET /v1/vin/batch "), "{text}");
    for line in lines {
        assert!(line.len() < 400, "a log line of {} bytes", line.len());
    }
}
