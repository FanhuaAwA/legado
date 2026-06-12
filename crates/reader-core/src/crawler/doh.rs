//! DNS-over-HTTPS resolver for the shared HTTP client, driven by the
//! `http_doh_server` app config key.
//!
//! Design notes:
//! - Uses the JSON DoH API (RFC 8484's `application/dns-json` variant), so no
//!   DNS wire-format codec / extra dependency is needed.
//! - The bootstrap client is pinned to each provider's well-known IP via
//!   `ClientBuilder::resolve`, so resolving the DoH server itself never recurses
//!   back into this resolver.
//! - **Fail-open**: any DoH error (network, unexpected JSON, unsupported
//!   provider response) falls back to the system resolver, so enabling DoH can
//!   never break name resolution — at worst it silently behaves like system DNS.

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use serde_json::Value;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Cache TTL for resolved records. DoH responses carry their own TTL, but a
/// fixed cap keeps the cache simple and bounded.
const CACHE_TTL: Duration = Duration::from_secs(300);
const DOH_TIMEOUT: Duration = Duration::from_secs(5);

struct DohProvider {
    /// DoH endpoint hostname (used for TLS SNI / cert validation).
    host: &'static str,
    /// Well-known IP the endpoint is reachable at, used to bootstrap without
    /// recursing into this resolver.
    ip: &'static str,
    /// Path of the JSON DoH endpoint.
    path: &'static str,
}

/// Map an `http_doh_server` config value to a provider. `none`/empty/unknown
/// yield `None` (system DNS). Keys mirror `DOH_OPTIONS` in SectionNetwork.vue.
fn provider_for(key: &str) -> Option<DohProvider> {
    Some(match key.trim() {
        "alidns" => DohProvider {
            host: "dns.alidns.com",
            ip: "223.5.5.5",
            path: "/resolve",
        },
        "dnspod" => DohProvider {
            host: "doh.pub",
            ip: "1.12.12.12",
            path: "/dns-query",
        },
        // 360 serves the JSON DoH API at /resolve; its /dns-query path only
        // accepts RFC 8484 wire-format (verified live 2026-06-12).
        "360dns" => DohProvider {
            host: "doh.360.cn",
            ip: "101.226.4.6",
            path: "/resolve",
        },
        "cloudflare" => DohProvider {
            host: "cloudflare-dns.com",
            ip: "1.1.1.1",
            path: "/dns-query",
        },
        "google" => DohProvider {
            host: "dns.google",
            ip: "8.8.8.8",
            path: "/resolve",
        },
        _ => return None,
    })
}

/// Parse the `Answer` array of a JSON DoH response into A/AAAA addresses.
/// Record type 1 = A, 28 = AAAA. Tolerant of missing/extra fields.
fn parse_doh_answers(json: &Value) -> Vec<IpAddr> {
    let Some(answers) = json.get("Answer").and_then(Value::as_array) else {
        return Vec::new();
    };
    answers
        .iter()
        .filter(|entry| {
            matches!(
                entry.get("type").and_then(Value::as_u64),
                Some(1) | Some(28)
            )
        })
        .filter_map(|entry| entry.get("data").and_then(Value::as_str))
        .filter_map(|data| data.trim().parse::<IpAddr>().ok())
        .collect()
}

fn to_addrs(ips: &[IpAddr]) -> Addrs {
    let socks: Vec<SocketAddr> = ips.iter().map(|ip| SocketAddr::new(*ip, 0)).collect();
    Box::new(socks.into_iter())
}

pub struct DohResolver {
    /// Bootstrap client pinned to the provider IP (no DoH recursion).
    client: reqwest::Client,
    endpoint: String,
    cache: Arc<RwLock<HashMap<String, (Vec<IpAddr>, Instant)>>>,
}

impl DohResolver {
    /// Build a resolver for the given `http_doh_server` value. Returns `None`
    /// for `none`/empty/unknown (caller should keep the default system DNS).
    pub fn from_config(key: &str, ignore_tls: bool) -> Option<Arc<Self>> {
        let provider = provider_for(key)?;
        let socket: SocketAddr = format!("{}:443", provider.ip).parse().ok()?;
        let mut builder = reqwest::Client::builder()
            .timeout(DOH_TIMEOUT)
            .resolve(provider.host, socket);
        if ignore_tls {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder.build().ok()?;
        Some(Arc::new(Self {
            client,
            endpoint: format!("https://{}{}", provider.host, provider.path),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }))
    }
}

async fn doh_query(client: &reqwest::Client, endpoint: &str, host: &str) -> Option<Vec<IpAddr>> {
    let response = client
        .get(endpoint)
        .query(&[("name", host), ("type", "A")])
        .header("accept", "application/dns-json")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: Value = response.json().await.ok()?;
    let ips = parse_doh_answers(&json);
    (!ips.is_empty()).then_some(ips)
}

impl Resolve for DohResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();
        let cache = self.cache.clone();
        Box::pin(async move {
            if let Some((ips, stamped)) = cache.read().await.get(&host) {
                if stamped.elapsed() < CACHE_TTL {
                    return Ok(to_addrs(ips));
                }
            }
            if let Some(ips) = doh_query(&client, &endpoint, &host).await {
                cache
                    .write()
                    .await
                    .insert(host.clone(), (ips.clone(), Instant::now()));
                return Ok(to_addrs(&ips));
            }
            // Fail open: fall back to the system resolver.
            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_mapping() {
        assert!(provider_for("none").is_none());
        assert!(provider_for("").is_none());
        assert!(provider_for("bogus").is_none());
        for key in ["alidns", "dnspod", "360dns", "cloudflare", "google"] {
            assert!(provider_for(key).is_some(), "missing provider for {key}");
        }
        // onedns has no public JSON DoH endpoint (live check 2026-06-12); removed.
        assert!(provider_for("onedns").is_none());
    }

    #[test]
    fn parses_a_and_aaaa_records() {
        let resp = json!({
            "Status": 0,
            "Answer": [
                { "name": "example.com.", "type": 1, "TTL": 300, "data": "93.184.216.34" },
                { "name": "example.com.", "type": 5, "TTL": 300, "data": "alias.example.com." },
                { "name": "example.com.", "type": 28, "TTL": 300, "data": "2606:2800:220:1:248:1893:25c8:1946" }
            ]
        });
        let ips = parse_doh_answers(&resp);
        assert_eq!(ips.len(), 2, "should keep A + AAAA, drop CNAME: {ips:?}");
        assert!(ips.iter().any(|ip| ip.is_ipv4()));
        assert!(ips.iter().any(|ip| ip.is_ipv6()));
    }

    #[test]
    fn parses_empty_or_missing_answer() {
        assert!(parse_doh_answers(&json!({ "Status": 2 })).is_empty());
        assert!(parse_doh_answers(&json!({ "Answer": [] })).is_empty());
        assert!(
            parse_doh_answers(&json!({ "Answer": [{ "type": 1, "data": "not-an-ip" }] }))
                .is_empty()
        );
    }

    #[test]
    fn builds_resolver_for_known_providers() {
        for key in ["cloudflare", "google", "alidns"] {
            assert!(DohResolver::from_config(key, false).is_some());
        }
        assert!(DohResolver::from_config("none", false).is_none());
    }

    /// Live verification (NET-004-LIVE): each provider must return a *real*
    /// JSON-DoH answer, not silently fail open to system DNS. `doh_query`
    /// returns `None` on any failure, so a non-empty result proves the bootstrap
    /// client + endpoint + IP pin actually reached the provider over DoH.
    /// Requires network; run with: `cargo test -p reader-core doh_live -- --ignored`.
    #[tokio::test]
    #[ignore = "live network"]
    async fn doh_live_each_provider_returns_real_answer() {
        for key in ["alidns", "dnspod", "360dns", "cloudflare", "google"] {
            let resolver = DohResolver::from_config(key, false).expect("provider builds");
            let ips = doh_query(&resolver.client, &resolver.endpoint, "www.example.com").await;
            assert!(
                ips.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
                "DoH provider {key} returned no real answer (would fail open to system DNS)"
            );
        }
    }

    /// Live verification for NET-005: the JS bridge uses a *blocking* reqwest
    /// client, but `DohResolver` is async (RwLock + async bootstrap client +
    /// `tokio::net::lookup_host`). This proves the async resolver runs correctly
    /// on the blocking client's internal runtime and actually fetches a page.
    /// Requires network; run with: `cargo test -p reader-core doh_live -- --ignored`.
    #[test]
    #[ignore = "live network"]
    fn doh_live_blocking_client_resolves_and_fetches() {
        let resolver = DohResolver::from_config("cloudflare", false).expect("provider builds");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .dns_resolver(resolver)
            .build()
            .expect("build blocking client with DoH resolver");
        let resp = client
            .get("https://www.example.com/")
            .send()
            .expect("blocking request resolved via async DoH on internal runtime");
        assert!(
            resp.status().is_success() || resp.status().is_redirection(),
            "unexpected status: {}",
            resp.status()
        );
    }
}
