//! The abuse ceiling: a limit on requests per minute from one address.
//!
//! It is held in memory and forgotten every minute. It exists so one client
//! cannot slow the service for everyone, not to meter use.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::state::AppState;

/// Most addresses counted in one minute. Beyond this, new addresses are let
/// through uncounted, so a flood from many addresses cannot use up memory.
const MOST_ADDRESSES: usize = 100_000;

/// Who a request is counted against: an IPv4 address, or the /64 network of
/// an IPv6 address, because one IPv6 customer has a whole /64 to send from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    V4(u32),
    V6(u64),
}

fn key(address: IpAddr) -> Key {
    match address {
        IpAddr::V4(v4) => Key::V4(v4.to_bits()),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => Key::V4(v4.to_bits()),
            None => Key::V6((v6.to_bits() >> 64) as u64),
        },
    }
}

#[derive(Debug, Default)]
struct Window {
    /// Whole minutes since the limiter was made.
    minute: u64,
    counts: HashMap<Key, u32>,
}

#[derive(Debug)]
pub struct Limiter {
    per_minute: u32,
    started: Instant,
    window: Mutex<Window>,
}

impl Limiter {
    pub fn new(per_minute: u32) -> Limiter {
        Limiter {
            per_minute,
            started: Instant::now(),
            window: Mutex::new(Window::default()),
        }
    }

    /// Counts one request. `Err` carries the seconds until the address may
    /// try again.
    pub fn check(&self, address: IpAddr, now: Instant) -> Result<(), u64> {
        let elapsed = now.saturating_duration_since(self.started).as_secs();
        let minute = elapsed / 60;
        let mut window = self.window.lock().unwrap_or_else(PoisonError::into_inner);
        if window.minute != minute {
            window.minute = minute;
            window.counts.clear();
        }
        let key = key(address);
        let tracked = window.counts.len();
        let count = match window.counts.get_mut(&key) {
            Some(count) => count,
            None if tracked >= MOST_ADDRESSES => return Ok(()),
            None => window.counts.entry(key).or_insert(0),
        };
        if *count >= self.per_minute {
            return Err(60 - elapsed % 60);
        }
        *count += 1;
        Ok(())
    }
}

/// The address a request came from.
///
/// With `trusted_proxies` of 0 this is the peer of the connection and
/// `X-Forwarded-For` is ignored, because anyone can send that header. With
/// one proxy in front, the proxy appends the address it saw, so the last
/// entry is the client and everything before it is whatever the client sent.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, trusted_proxies: usize) -> IpAddr {
    let fallback = peer.unwrap_or(IpAddr::from([0, 0, 0, 0]));
    if trusted_proxies == 0 {
        return fallback;
    }
    let entries: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    entries
        .len()
        .checked_sub(trusted_proxies)
        .and_then(|index| entries.get(index))
        .and_then(|entry| entry.parse::<IpAddr>().ok())
        .unwrap_or(fallback)
}

/// Refuses a request from an address that is over the limit. `/health` is
/// never counted: the deploy proxy calls it every few seconds.
pub async fn limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    let address = client_ip(request.headers(), peer, state.config().trusted_proxies);
    match state.limiter().check(address, Instant::now()) {
        Ok(()) => next.run(request).await,
        Err(retry_after) => {
            tracing::warn!(%address, "over the abuse ceiling");
            ApiError::RateLimited { retry_after }.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn address(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn headers(values: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append("x-forwarded-for", value.parse().unwrap());
        }
        headers
    }

    #[test]
    fn the_request_after_the_limit_is_refused_until_the_minute_ends() {
        let limiter = Limiter::new(600);
        let start = limiter.started;
        let client = address("203.0.113.7");
        for _ in 0..600 {
            assert_eq!(
                limiter.check(client, start + Duration::from_secs(10)),
                Ok(())
            );
        }
        assert_eq!(
            limiter.check(client, start + Duration::from_secs(10)),
            Err(50)
        );
        assert_eq!(
            limiter.check(client, start + Duration::from_secs(59)),
            Err(1)
        );
        assert_eq!(
            limiter.check(client, start + Duration::from_secs(60)),
            Ok(())
        );
    }

    #[test]
    fn one_address_over_the_limit_does_not_affect_another() {
        let limiter = Limiter::new(2);
        let now = limiter.started;
        assert_eq!(limiter.check(address("203.0.113.7"), now), Ok(()));
        assert_eq!(limiter.check(address("203.0.113.7"), now), Ok(()));
        assert!(limiter.check(address("203.0.113.7"), now).is_err());
        assert_eq!(limiter.check(address("203.0.113.8"), now), Ok(()));
    }

    #[test]
    fn an_ipv6_network_is_counted_as_one_client() {
        let limiter = Limiter::new(2);
        let now = limiter.started;
        assert_eq!(limiter.check(address("2001:db8:1:2::1"), now), Ok(()));
        assert_eq!(limiter.check(address("2001:db8:1:2::2"), now), Ok(()));
        assert!(limiter.check(address("2001:db8:1:2:ffff::3"), now).is_err());
        assert_eq!(limiter.check(address("2001:db8:1:3::1"), now), Ok(()));
        // An IPv4 address written as IPv6 is the same client as the IPv4 one.
        assert_eq!(limiter.check(address("::ffff:203.0.113.7"), now), Ok(()));
        assert_eq!(limiter.check(address("203.0.113.7"), now), Ok(()));
        assert!(limiter.check(address("203.0.113.7"), now).is_err());
    }

    #[test]
    fn a_clock_that_goes_backwards_does_not_panic() {
        let limiter = Limiter::new(1);
        let earlier = limiter.started.checked_sub(Duration::from_secs(5));
        if let Some(earlier) = earlier {
            assert_eq!(limiter.check(address("203.0.113.7"), earlier), Ok(()));
        }
    }

    #[test]
    fn without_a_proxy_the_forwarded_header_is_ignored() {
        let peer = address("198.51.100.9");
        let spoofed = headers(&["203.0.113.7"]);
        assert_eq!(client_ip(&spoofed, Some(peer), 0), peer);
    }

    #[test]
    fn behind_one_proxy_only_the_last_entry_counts() {
        let proxy = address("172.18.0.2");
        // The client sent the first entry itself; the proxy added the second.
        let spoofed = headers(&["10.0.0.1, 203.0.113.7"]);
        assert_eq!(client_ip(&spoofed, Some(proxy), 1), address("203.0.113.7"));
        // The same, with the header sent on two lines.
        let two_lines = headers(&["10.0.0.1", "203.0.113.7"]);
        assert_eq!(
            client_ip(&two_lines, Some(proxy), 1),
            address("203.0.113.7")
        );
        let v6 = headers(&["2001:db8::5"]);
        assert_eq!(client_ip(&v6, Some(proxy), 1), address("2001:db8::5"));
    }

    #[test]
    fn a_missing_or_unreadable_header_falls_back_to_the_peer() {
        let proxy = address("172.18.0.2");
        assert_eq!(client_ip(&HeaderMap::new(), Some(proxy), 1), proxy);
        assert_eq!(
            client_ip(&headers(&["not an address"]), Some(proxy), 1),
            proxy
        );
        assert_eq!(client_ip(&headers(&["203.0.113.7"]), Some(proxy), 2), proxy);
        assert_eq!(client_ip(&headers(&[", ,"]), Some(proxy), 1), proxy);
        assert_eq!(
            client_ip(&HeaderMap::new(), None, 1),
            IpAddr::from([0, 0, 0, 0])
        );
    }
}
