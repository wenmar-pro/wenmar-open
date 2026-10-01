//! What the server writes to its log, through the whole application.

use axum::body::Body;
use axum::http::Request;

use crate::capture::Capture;
use crate::common;

/// A real VIN, as the final review sent it. Its serial number is `004352`.
const VIN: &str = "1HGCM82633A004352";

#[tokio::test]
async fn no_address_puts_a_whole_vin_in_the_log() {
    let app = common::app().await;
    let captured = Capture::start();

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

/// The request lines this thread's capture holds.
fn request_lines(captured: &Capture) -> Vec<String> {
    captured
        .text()
        .lines()
        .filter(|line| line.contains("open_server::log"))
        .map(str::to_owned)
        .collect()
}

/// A request made on a thread of its own, as another test's would be.
fn request_on_another_thread(path: &'static str) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                common::app().await.get(path).await;
            });
    })
    .join()
    .unwrap();
}

// Whether a log statement writes anything is decided once for the whole
// process, by the first thread that reaches it. Tests run on many threads,
// so a capture must not depend on which test reached the statement first.
#[tokio::test]
async fn a_capture_holds_this_threads_lines_whoever_logged_first() {
    let app = common::app().await;
    let captured = Capture::start();
    request_on_another_thread("/v1/somewhere-else");
    app.get("/v1/meta").await;
    request_on_another_thread("/v1/somewhere-else");

    let lines = request_lines(&captured);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("GET /v1/meta 200"), "{lines:?}");
}

#[tokio::test]
async fn a_capture_ends_when_it_is_dropped() {
    let app = common::app().await;
    let first = Capture::start();
    app.get("/v1/meta").await;
    assert_eq!(request_lines(&first).len(), 1);
    drop(first);

    app.get("/v1/meta").await;
    let second = Capture::start();
    assert_eq!(request_lines(&second), Vec::<String>::new());
    app.get("/v1/years-of-nothing").await;
    assert_eq!(request_lines(&second).len(), 1);
}
