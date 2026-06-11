//! Verifies that the shared HTTP client actually consumes the Network settings
//! config keys (proxy / UA / TLS / timeouts / redirects). The Network UI saves
//! booleans and numbers as strings, so parsing must accept both forms.

use reader_core::crawler::http_client::{HttpClient, HttpClientConfig, DEFAULT_USER_AGENT};
use serde_json::json;

#[test]
fn empty_config_yields_defaults() {
    let cfg = HttpClientConfig::from_app_config(&json!({}), 30);
    assert_eq!(cfg.user_agent, DEFAULT_USER_AGENT);
    assert_eq!(cfg.request_timeout_secs, 30);
    assert_eq!(cfg.connect_timeout_secs, 10);
    assert!(cfg.follow_redirects);
    assert!(!cfg.ignore_tls_errors);
    assert_eq!(cfg.proxy_mode, "system");
}

#[test]
fn parses_string_encoded_values_from_ui() {
    // The Vue Network panel persists numbers/booleans via String(v).
    let value = json!({
        "http_user_agent": "CustomUA/1.0",
        "http_follow_redirects": "false",
        "http_ignore_tls_errors": "true",
        "http_connect_timeout_secs": "20",
        "proxy_mode": "custom",
        "proxy_type": "socks5",
        "proxy_host": "127.0.0.1",
        "proxy_port": "1080",
        "proxy_username": "user",
        "proxy_password": "pass",
    });
    let cfg = HttpClientConfig::from_app_config(&value, 30);
    assert_eq!(cfg.user_agent, "CustomUA/1.0");
    assert!(!cfg.follow_redirects);
    assert!(cfg.ignore_tls_errors);
    assert_eq!(cfg.connect_timeout_secs, 20);
    assert_eq!(cfg.proxy_mode, "custom");
    assert_eq!(cfg.proxy_type, "socks5");
    assert_eq!(cfg.proxy_host, "127.0.0.1");
    assert_eq!(cfg.proxy_port, 1080);
}

#[test]
fn parses_json_typed_values() {
    let value = json!({
        "http_follow_redirects": true,
        "http_ignore_tls_errors": false,
        "http_connect_timeout_secs": 15,
        "proxy_port": 8080,
    });
    let cfg = HttpClientConfig::from_app_config(&value, 30);
    assert!(cfg.follow_redirects);
    assert!(!cfg.ignore_tls_errors);
    assert_eq!(cfg.connect_timeout_secs, 15);
    assert_eq!(cfg.proxy_port, 8080);
}

#[test]
fn blank_user_agent_falls_back_to_default() {
    let cfg = HttpClientConfig::from_app_config(&json!({ "http_user_agent": "   " }), 30);
    assert_eq!(cfg.user_agent, DEFAULT_USER_AGENT);
}

#[test]
fn builds_clients_for_every_proxy_mode() {
    for mode in ["system", "none", "custom"] {
        let value = json!({
            "proxy_mode": mode,
            "proxy_type": "http",
            "proxy_host": "127.0.0.1",
            "proxy_port": 8080,
        });
        let cfg = HttpClientConfig::from_app_config(&value, 30);
        assert!(
            HttpClient::from_config(&cfg).is_ok(),
            "failed to build client for proxy_mode={mode}"
        );
    }
}

#[test]
fn builds_socks5_proxy_client() {
    let value = json!({
        "proxy_mode": "custom",
        "proxy_type": "socks5",
        "proxy_host": "127.0.0.1",
        "proxy_port": 1080,
        "proxy_username": "u",
        "proxy_password": "p",
    });
    let cfg = HttpClientConfig::from_app_config(&value, 30);
    assert!(HttpClient::from_config(&cfg).is_ok());
}

#[test]
fn custom_proxy_without_host_or_port_is_skipped() {
    // proxy_mode=custom but host/port unset must still build (no proxy applied).
    let cfg = HttpClientConfig::from_app_config(&json!({ "proxy_mode": "custom" }), 30);
    assert!(HttpClient::from_config(&cfg).is_ok());
}

#[test]
fn doh_server_is_parsed_and_client_builds() {
    let none = HttpClientConfig::from_app_config(&json!({}), 30);
    assert_eq!(none.doh_server, "none");
    assert!(HttpClient::from_config(&none).is_ok());

    // A known DoH provider parses and the client builds (resolver attached).
    let cfg = HttpClientConfig::from_app_config(&json!({ "http_doh_server": "cloudflare" }), 30);
    assert_eq!(cfg.doh_server, "cloudflare");
    assert!(HttpClient::from_config(&cfg).is_ok());

    // An unknown provider is harmless (falls back to system DNS, still builds).
    let bogus = HttpClientConfig::from_app_config(&json!({ "http_doh_server": "bogus" }), 30);
    assert!(HttpClient::from_config(&bogus).is_ok());
}
