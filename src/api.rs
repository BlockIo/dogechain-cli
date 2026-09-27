//! HTTP client for the Dogechain.com JSON API (/api/v3).

use std::io::{BufRead, BufReader};
use std::thread::sleep;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::Url;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT, RETRY_AFTER};
use serde_json::Value;

use crate::error::{CliError, Result};

/// The only API the CLI talks to.
pub const API_BASE: &str = "https://dogechain.com";

/// How many times a rate-limited request is retried, and the longest
/// Retry-After the CLI will sit out before giving up.
const RATE_LIMIT_RETRIES: u32 = 3;
const MAX_RETRY_WAIT_SECS: u64 = 30;

pub struct Api {
    base: Url,
    http: Client,
    timeout: Duration,
}

impl Api {
    pub fn new(timeout: Duration) -> Result<Api> {
        let base = Url::parse(&base_url())
            .map_err(|e| CliError::Other(format!("invalid API URL: {e}")))?;
        let http = Client::builder()
            .user_agent(concat!("dogechain-cli/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(timeout)
            .build()
            .map_err(|e| CliError::Other(format!("could not start the HTTP client: {e}")))?;
        Ok(Api {
            base,
            http,
            timeout,
        })
    }

    /// `/api/v3/<segments…>`, each segment percent-encoded so user input
    /// cannot change the path.
    fn url(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("API base URL has a path")
            .pop_if_empty()
            .extend(["api", "v3"])
            .extend(segments);
        url
    }

    /// GETs an endpoint and returns the whole success envelope
    /// (`{"status":"success","data":…}`). Waits out rate limits as the API asks.
    pub fn get(&self, segments: &[&str], query: &[(&str, String)]) -> Result<Value> {
        let mut url = self.url(segments);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let resp = self.send_with_retry(|| self.http.get(url.clone()).timeout(self.timeout))?;
        let status = resp.status();
        let body: Option<Value> = resp.json().ok();
        let message = body
            .as_ref()
            .and_then(|b| b["data"]["error_message"].as_str())
            .map(str::to_owned);

        let Some(body) = body else {
            // Something answered, but not the API: e.g. a web page or proxy
            // error in front of it.
            return Err(CliError::Unavailable(format!(
                "dogechain.com did not return API data (HTTP {}); try again later",
                status.as_u16()
            )));
        };
        match status {
            s if s.is_success() && body["status"] == "success" => Ok(body),
            s if s.is_success() => {
                Err(CliError::Other(message.unwrap_or_else(|| {
                    "unexpected response from dogechain.com".into()
                })))
            }
            StatusCode::BAD_REQUEST => {
                Err(CliError::BadInput(message.unwrap_or_else(|| {
                    "dogechain.com rejected the request".into()
                })))
            }
            StatusCode::NOT_FOUND => Err(CliError::NotFound(
                message.unwrap_or_else(|| "not found".into()),
            )),
            s if is_unavailable(s) => Err(CliError::Unavailable(message.unwrap_or_else(|| {
                format!(
                    "dogechain.com is unavailable right now (HTTP {})",
                    s.as_u16()
                )
            }))),
            s => Err(CliError::Other(message.unwrap_or_else(|| {
                format!("dogechain.com returned HTTP {}", s.as_u16())
            }))),
        }
    }

    /// Opens the live event stream (`/api/v3/live`, server-sent events) and
    /// calls `on_event(event, data)` for each event until the stream ends or
    /// `on_event` returns false.
    pub fn live(&self, mut on_event: impl FnMut(&str, &str) -> bool) -> Result<()> {
        let url = self.url(&["live"]);
        // No overall timeout: the stream is meant to stay open.
        let resp = self.send_with_retry(|| {
            self.http
                .get(url.clone())
                .header(ACCEPT, "text/event-stream")
        })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(if is_unavailable(status) {
                CliError::Unavailable(format!(
                    "the live stream is unavailable right now (HTTP {})",
                    status.as_u16()
                ))
            } else {
                CliError::Other(format!("live stream: HTTP {}", status.as_u16()))
            });
        }

        let mut event = String::new();
        let mut data = String::new();
        for line in BufReader::new(resp).lines() {
            let line = line.map_err(|e| {
                CliError::Unavailable(format!("the live stream was interrupted: {e}"))
            })?;
            if line.is_empty() {
                if !data.is_empty() {
                    let name = if event.is_empty() { "message" } else { &event };
                    if !on_event(name, &data) {
                        return Ok(());
                    }
                }
                event.clear();
                data.clear();
            } else if let Some(v) = line.strip_prefix("event:") {
                event = v.trim_start().to_owned();
            } else if let Some(v) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(v.strip_prefix(' ').unwrap_or(v));
            }
            // Comments (":…"), `id:` and `retry:` lines are not needed.
        }
        Err(CliError::Unavailable("the live stream ended".into()))
    }

    fn send_with_retry(&self, request: impl Fn() -> RequestBuilder) -> Result<Response> {
        let mut attempt = 0;
        loop {
            let resp = request().send().map_err(|e| unreachable_error(&e))?;
            if resp.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(resp);
            }
            let wait = retry_after(&resp).unwrap_or(1);
            attempt += 1;
            if attempt > RATE_LIMIT_RETRIES || wait > MAX_RETRY_WAIT_SECS {
                return Err(CliError::RateLimited {
                    retry_after_secs: wait,
                });
            }
            eprintln!("dogechain: rate limited; retrying in {wait} s");
            sleep(Duration::from_secs(wait));
        }
    }
}

/// Tests point the binary at a local server. Only debug builds read this, so
/// released binaries always talk to dogechain.com.
fn base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(base) = std::env::var("DOGECHAIN_TEST_API_BASE") {
        return base;
    }
    API_BASE.to_owned()
}

fn is_unavailable(s: StatusCode) -> bool {
    matches!(
        s,
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
    )
}

/// Seconds from a Retry-After header in its delay form.
fn retry_after(resp: &Response) -> Option<u64> {
    resp.headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn unreachable_error(e: &reqwest::Error) -> CliError {
    let what = if e.is_timeout() {
        "timed out waiting for dogechain.com".to_owned()
    } else {
        "could not reach dogechain.com".to_owned()
    };
    // The innermost cause is the useful part ("connection refused", DNS, TLS).
    let mut cause: &dyn std::error::Error = e;
    while let Some(next) = cause.source() {
        cause = next;
    }
    CliError::Unavailable(format!("{what}: {cause}"))
}
