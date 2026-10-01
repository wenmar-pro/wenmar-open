//! A small HTTP server on this computer, standing in for the hosted API
//! and for the release server. Tests never use the network.

// Not every test module uses every helper.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// A request the server received.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    /// The path and query, as sent.
    pub target: String,
    /// Header names in lowercase, with their values.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// What the server answers.
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    /// Say the body is this long, whatever its real length. For a
    /// connection that drops part way.
    pub claimed_length: Option<usize>,
}

impl Reply {
    pub fn json(status: u16, body: &serde_json::Value) -> Reply {
        Reply {
            status,
            content_type: "application/json",
            body: body.to_string().into_bytes(),
            claimed_length: None,
        }
    }

    pub fn html(status: u16, body: &str) -> Reply {
        Reply {
            status,
            content_type: "text/html; charset=utf-8",
            body: body.as_bytes().to_vec(),
            claimed_length: None,
        }
    }

    pub fn bytes(body: Vec<u8>) -> Reply {
        Reply {
            status: 200,
            content_type: "application/gzip",
            body,
            claimed_length: None,
        }
    }
}

/// A handler in a box, for a list of them.
pub type Handler = Box<dyn Fn(&Seen) -> Reply + Send>;

pub struct Server {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    /// The server's address, such as `http://127.0.0.1:49152`.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Every request so far, in order.
    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    /// The path and query of every request so far.
    pub fn targets(&self) -> Vec<String> {
        self.seen().into_iter().map(|seen| seen.target).collect()
    }
}

fn read_request(stream: &TcpStream) -> Option<Seen> {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_lowercase(), value.trim().to_owned()));
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Seen {
        method,
        target,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Starts a server that answers every request with `handler`. It runs
/// until the test process ends.
pub fn serve(handler: impl Fn(&Seen) -> Reply + Send + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Some(request) = read_request(&stream) else {
                continue;
            };
            let reply = handler(&request);
            record.lock().unwrap().push(request);
            let head = format!(
                "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reply.status,
                reply.content_type,
                reply.claimed_length.unwrap_or(reply.body.len())
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&reply.body);
            let _ = stream.flush();
        }
    });
    Server { url, seen }
}

/// An address on this computer that nothing is listening on.
pub fn nothing_listening() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}
