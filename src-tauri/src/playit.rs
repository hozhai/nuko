use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, ChildStdout, Command, Stdio},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use reqwest::{header, Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use tauri::async_runtime;
use tokio::time::sleep;

use crate::models::PlayitTunnelMetadata;

const API_BASE: &str = "https://api.playit.gg";
const RUN_DATA_PATH: &str = "/v1/agents/rundata";
const LEGACY_RUN_DATA_PATH: &str = "/agents/rundata";
const USER_AGENT: &str = "nuko-playit/0.1";
const AGENT_TYPE: &str = "self-managed";
const AGENT_VERSION: &str = "0.15.13";
const CLAIM_CODE_TIMEOUT_SECS: u64 = 60;
const CLAIM_DETAILS_MAX_ATTEMPTS: usize = 60;
const CLAIM_EXCHANGE_MAX_ATTEMPTS: usize = 30;

/// Lightweight helper for querying the playit.gg agent API using an agent secret.
#[derive(Clone)]
pub struct PlayitClient {
    http: Client,
    secret: String,
    base_url: String,
}

impl PlayitClient {
    /// Create a new client from an agent secret.
    pub fn new(secret: impl Into<String>) -> Result<Self, String> {
        let trimmed = secret.into();
        let trimmed = trimmed.trim();
        if trimmed.is_empty() {
            return Err("Playit secret cannot be empty".into());
        }

        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| format!("Failed to build Playit HTTP client: {e}"))?;

        Ok(Self {
            http,
            secret: trimmed.to_string(),
            base_url: API_BASE.to_string(),
        })
    }

    /// Override the API base URL (useful for tests or regional endpoints).
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.base_url = base.into();
        self
    }

    /// Fetch the current tunnels registered to this agent.
    pub async fn fetch_tunnels(&self) -> Result<Vec<PlayitTunnelMetadata>, String> {
        match self.fetch_tunnels_v1().await {
            Ok(tunnels) => Ok(tunnels),
            Err((should_try_legacy, err)) => {
                if should_try_legacy {
                    match self.fetch_tunnels_legacy().await {
                        Ok(tunnels) => Ok(tunnels),
                        Err(legacy_err) => Err(format!(
                            "Playit tunnel fetch failed (v1: {err}; legacy: {legacy_err})"
                        )),
                    }
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn fetch_tunnels_v1(&self) -> Result<Vec<PlayitTunnelMetadata>, (bool, String)> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, RUN_DATA_PATH))
            .header(
                header::AUTHORIZATION,
                format!("Agent-Key {}", self.secret.trim()),
            )
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| (false, format!("Playit request failed: {e}")))?;

        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|e| (false, format!("Failed to read Playit response body: {e}")))?;

        if status == StatusCode::OK {
            let envelope: ApiEnvelope<AgentRunDataV1> =
                serde_json::from_slice(&body).map_err(|e| {
                    (
                        false,
                        format!(
                            "Failed to parse Playit response: {e}. Body: {}",
                            body_snippet(&body)
                        ),
                    )
                })?;

            let data = match envelope {
                ApiEnvelope::Success { data } => data,
                ApiEnvelope::Fail { data } => {
                    return Err((false, format!("Playit API reported failure: {data:?}")));
                }
                ApiEnvelope::Error { error } => {
                    return Err((
                        false,
                        format!("Playit API internal error: {}", error.message()),
                    ));
                }
            };

            return Ok(data.into_metadata());
        }

        let snippet = body_snippet(&body);
        let fallback_status = matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        );

        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err((
                false,
                "Playit rejected the saved secret. Restart this instance to regenerate a new Playit agent token."
                    .into(),
            )),
            StatusCode::TOO_MANY_REQUESTS => Err((
                false,
                "Playit rate-limited the request. Please wait a few seconds before trying again."
                    .into(),
            )),
            _ => Err((
                fallback_status,
                format!("Playit responded with {}: {}", status, snippet),
            )),
        }
    }

    async fn fetch_tunnels_legacy(&self) -> Result<Vec<PlayitTunnelMetadata>, String> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, LEGACY_RUN_DATA_PATH))
            .header(
                header::AUTHORIZATION,
                format!("Agent-Key {}", self.secret.trim()),
            )
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| format!("Playit legacy request failed: {e}"))?;

        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read Playit legacy response body: {e}"))?;

        if status == StatusCode::OK {
            let envelope: ApiEnvelope<LegacyAgentRunData> =
                serde_json::from_slice(&body).map_err(|e| {
                    format!(
                        "Failed to parse Playit legacy response: {e}. Body: {}",
                        body_snippet(&body)
                    )
                })?;

            let data = match envelope {
                ApiEnvelope::Success { data } => data,
                ApiEnvelope::Fail { data } => {
                    return Err(format!("Playit legacy API reported failure: {data:?}"));
                }
                ApiEnvelope::Error { error } => {
                    return Err(format!(
                        "Playit legacy API internal error: {}",
                        error.message()
                    ));
                }
            };

            return Ok(data.into_metadata());
        }

        let snippet = body_snippet(&body);
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(
                "Playit rejected the saved secret. Restart this instance to regenerate a new Playit agent token."
                    .into(),
            ),
            StatusCode::TOO_MANY_REQUESTS => Err(
                "Playit rate-limited the request. Please wait a few seconds before trying again."
                    .into(),
            ),
            _ => Err(format!(
                "Playit legacy endpoint responded with {}: {}",
                status, snippet
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum ApiEnvelope<T> {
    Success { data: T },
    Fail { data: serde_json::Value },
    Error { error: ApiErrorPayload },
}

#[derive(Debug, Deserialize)]
struct ApiErrorPayload {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

impl ApiErrorPayload {
    fn message(&self) -> String {
        self.message
            .clone()
            .or_else(|| self.code.clone())
            .unwrap_or_else(|| "Unknown error".into())
    }
}

#[derive(Debug, Deserialize)]
struct AgentRunDataV1 {
    #[serde(default)]
    tunnels: Vec<AgentTunnelV1>,
}

impl AgentRunDataV1 {
    fn into_metadata(self) -> Vec<PlayitTunnelMetadata> {
        self.tunnels.into_iter().map(|t| t.into()).collect()
    }
}

#[derive(Debug, Deserialize)]
struct AgentTunnelV1 {
    id: String,
    name: String,
    display_address: String,
    #[serde(default)]
    tunnel_type_display: Option<String>,
    #[serde(default)]
    disabled_reason: Option<String>,
    #[serde(default)]
    port_type: Option<String>,
    #[serde(default)]
    agent_config: AgentTunnelConfig,
}

impl From<AgentTunnelV1> for PlayitTunnelMetadata {
    fn from(value: AgentTunnelV1) -> Self {
        let AgentTunnelV1 {
            id,
            name,
            display_address,
            tunnel_type_display,
            disabled_reason,
            port_type,
            agent_config,
        } = value;

        let (public_hostname, public_port) = parse_address(&display_address);
        let destination_port = agent_config
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case("port") || f.name.contains("port"))
            .and_then(|f| f.value.parse::<u16>().ok());

        PlayitTunnelMetadata {
            id: Some(id),
            name: if name.trim().is_empty() {
                tunnel_type_display
            } else {
                Some(name.trim().to_string())
            },
            protocol: port_type.map(|p| p.to_uppercase()),
            public_hostname,
            public_port,
            destination_port,
            agent_version: None,
            status: Some(
                disabled_reason
                    .map(|reason| format!("Disabled ({reason})"))
                    .unwrap_or_else(|| "Active".into()),
            ),
            last_heartbeat: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacyAgentRunData {
    #[serde(default)]
    tunnels: Vec<LegacyAgentTunnel>,
}

impl LegacyAgentRunData {
    fn into_metadata(self) -> Vec<PlayitTunnelMetadata> {
        self.tunnels.into_iter().map(|t| t.into()).collect()
    }
}

#[derive(Debug, Deserialize)]
struct LegacyAgentTunnel {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tunnel_type: Option<String>,
    #[serde(default)]
    assigned_domain: Option<String>,
    #[serde(default)]
    custom_domain: Option<String>,
    #[serde(default)]
    proto: Option<String>,
    #[serde(default)]
    port: LegacyPortRange,
    #[serde(default)]
    disabled: Option<String>,
    #[serde(default)]
    agent_config: AgentTunnelConfig,
}

#[derive(Debug, Default, Deserialize)]
struct LegacyPortRange {
    #[serde(default)]
    from: u16,
    #[serde(default)]
    to: u16,
}

impl From<LegacyAgentTunnel> for PlayitTunnelMetadata {
    fn from(value: LegacyAgentTunnel) -> Self {
        let LegacyAgentTunnel {
            id,
            name,
            tunnel_type,
            assigned_domain,
            custom_domain,
            proto,
            port,
            disabled,
            agent_config,
        } = value;

        let display_name = name
            .as_deref()
            .and_then(|n| {
                let trimmed = n.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .or(tunnel_type);

        let public_hostname = custom_domain
            .filter(|s| !s.trim().is_empty())
            .or_else(|| assigned_domain.filter(|s| !s.trim().is_empty()));

        let public_port = if port.from == 0 {
            None
        } else {
            Some(port.from)
        };

        let destination_port = agent_config
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case("port") || f.name.contains("port"))
            .and_then(|f| f.value.parse::<u16>().ok());

        PlayitTunnelMetadata {
            id: Some(id),
            name: display_name,
            protocol: proto.map(|p| p.to_uppercase()),
            public_hostname,
            public_port,
            destination_port,
            agent_version: None,
            status: Some(
                disabled
                    .map(|reason| format!("Disabled ({reason})"))
                    .unwrap_or_else(|| "Active".into()),
            ),
            last_heartbeat: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct AgentTunnelConfig {
    #[serde(default)]
    fields: Vec<AgentTunnelField>,
}

#[derive(Debug, Deserialize)]
struct AgentTunnelField {
    name: String,
    value: String,
}

pub async fn fetch_playit_tunnels(secret: &str) -> Result<Vec<PlayitTunnelMetadata>, String> {
    PlayitClient::new(secret)?.fetch_tunnels().await
}

fn parse_address(address: &str) -> (Option<String>, Option<u16>) {
    let trimmed = address.trim();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);

    if without_scheme.starts_with('[') {
        if let Some(end) = without_scheme.rfind(']') {
            let host = without_scheme[..=end].to_string();
            let port = without_scheme[end + 1..]
                .strip_prefix(':')
                .and_then(|segment| segment.parse::<u16>().ok());
            return (Some(host), port);
        }
    }

    if let Some((host, port_str)) = without_scheme.rsplit_once(':') {
        if port_str.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(port) = port_str.parse::<u16>() {
                return (Some(host.to_string()), Some(port));
            }
        }
    }

    (Some(without_scheme.to_string()), None)
}

fn body_snippet(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "«empty»".into();
    }
    let mut snippet: String = trimmed.chars().take(200).collect();
    if trimmed.len() > 200 {
        snippet.push_str("...");
    }
    snippet
}

pub async fn claim_playit_secret(
    playit_path: &Path,
    working_dir: &Path,
    secret_path: &Path,
) -> Result<String, String> {
    let mut cmd = Command::new(playit_path);
    cmd.current_dir(working_dir);
    cmd.arg("-s");
    cmd.arg("--secret_path");
    cmd.arg(secret_path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch playit agent for claim: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture playit agent stdout".to_string())?;

    let code_rx = spawn_claim_listener(stdout);

    let claim_code = match async_runtime::spawn_blocking(move || {
        code_rx.recv_timeout(Duration::from_secs(CLAIM_CODE_TIMEOUT_SECS))
    })
    .await
    {
        Ok(Ok(code)) => code,
        Ok(Err(RecvTimeoutError::Timeout)) => {
            terminate_child(&mut child);
            return Err("Timed out waiting for playit claim link".into());
        }
        Ok(Err(RecvTimeoutError::Disconnected)) => {
            terminate_child(&mut child);
            return Err("Playit agent exited before printing a claim link".into());
        }
        Err(err) => {
            terminate_child(&mut child);
            return Err(format!("Failed waiting for playit claim link: {err}"));
        }
    };

    let result = exchange_claim_code(&claim_code).await;

    terminate_child(&mut child);

    result
}

fn spawn_claim_listener(stdout: ChildStdout) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if let Some(code) = extract_claim_code(&line) {
                        let _ = tx.send(code);
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    rx
}

fn extract_claim_code(line: &str) -> Option<String> {
    const CLAIM_PREFIX: &str = "https://playit.gg/claim/";
    let start = line.find(CLAIM_PREFIX)? + CLAIM_PREFIX.len();
    let code: String = line[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if code.is_empty() {
        None
    } else {
        Some(code.to_lowercase())
    }
}

fn terminate_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn exchange_claim_code(claim_code: &str) -> Result<String, String> {
    let normalized = claim_code.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("Playit claim code is empty".into());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to build Playit HTTP client: {e}"))?;

    let guest = post_envelope(&client, "/login/create/guest", None, json!({})).await?;
    let session_key = match guest {
        ApiEnvelope::Success { data } => data
            .get("session_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Playit guest session missing session_key".to_string())?
            .to_string(),
        ApiEnvelope::Fail { data } => {
            return Err(format!("Playit guest session failed: {data:?}"));
        }
        ApiEnvelope::Error { error } => {
            return Err(format!("Playit guest session error: {}", error.message()));
        }
    };

    let auth = format!("Bearer {}", session_key);

    wait_for_claim_details(&client, &auth, &normalized).await?;
    send_claim_setup(&client, &auth, &normalized).await?;
    send_claim_accept(&client, &auth, &normalized).await?;
    wait_for_claim_exchange(&client, &auth, &normalized).await
}

async fn wait_for_claim_details(client: &Client, auth: &str, code: &str) -> Result<(), String> {
    let payload = json!({
        "code": code,
        "agent_type": AGENT_TYPE,
        "version": AGENT_VERSION,
    });

    for _ in 0..CLAIM_DETAILS_MAX_ATTEMPTS {
        match post_envelope(client, "/claim/details", Some(auth), payload.clone()).await? {
            ApiEnvelope::Success { .. } => return Ok(()),
            ApiEnvelope::Fail { data } => {
                if let Some(reason) = data.as_str() {
                    match reason {
                        "WaitingForAgent" => sleep(Duration::from_secs(1)).await,
                        "CodeNotFound" | "ClaimExpired" => {
                            return Err(format!(
                                "Playit claim/details rejected the code: {reason}"
                            ));
                        }
                        other => return Err(format!("Playit claim/details failed: {other}")),
                    }
                } else {
                    return Err(format!("Playit claim/details failed: {data:?}"));
                }
            }
            ApiEnvelope::Error { error } => {
                return Err(format!("Playit claim/details error: {}", error.message()))
            }
        }
    }

    Err("Timed out waiting for Playit agent to register the claim code".into())
}

async fn send_claim_setup(client: &Client, auth: &str, code: &str) -> Result<(), String> {
    let payload = json!({
        "code": code,
        "agent_type": AGENT_TYPE,
        "version": AGENT_VERSION,
    });

    match post_envelope(client, "/claim/setup", Some(auth), payload).await? {
        ApiEnvelope::Success { .. } => Ok(()),
        ApiEnvelope::Fail { data } => Err(format!("Playit claim/setup failed: {data:?}")),
        ApiEnvelope::Error { error } => {
            Err(format!("Playit claim/setup error: {}", error.message()))
        }
    }
}

async fn send_claim_accept(client: &Client, auth: &str, code: &str) -> Result<(), String> {
    let alias: String = format!("nuko-{}", code.chars().take(4).collect::<String>());
    let payload = json!({
        "code": code,
        "name": alias,
        "agent_type": AGENT_TYPE,
    });

    match post_envelope(client, "/claim/accept", Some(auth), payload).await? {
        ApiEnvelope::Success { .. } => Ok(()),
        ApiEnvelope::Fail { data } => Err(format!("Playit claim/accept failed: {data:?}")),
        ApiEnvelope::Error { error } => {
            Err(format!("Playit claim/accept error: {}", error.message()))
        }
    }
}

async fn wait_for_claim_exchange(
    client: &Client,
    auth: &str,
    code: &str,
) -> Result<String, String> {
    let payload = json!({ "code": code });

    for _ in 0..CLAIM_EXCHANGE_MAX_ATTEMPTS {
        match post_envelope(client, "/claim/exchange", Some(auth), payload.clone()).await? {
            ApiEnvelope::Success { data } => {
                if let Some(secret) = data.get("secret_key").and_then(|v| v.as_str()) {
                    return Ok(secret.to_string());
                }
                return Err("Playit claim/exchange response missing secret_key".into());
            }
            ApiEnvelope::Fail { data } => {
                if let Some(reason) = data.as_str() {
                    match reason {
                        "NotAccepted" => sleep(Duration::from_secs(1)).await,
                        "CodeNotFound" | "ClaimExpired" => {
                            return Err(format!(
                                "Playit claim/exchange rejected the code: {reason}"
                            ));
                        }
                        other => return Err(format!("Playit claim/exchange failed: {other}")),
                    }
                } else {
                    return Err(format!("Playit claim/exchange failed: {data:?}"));
                }
            }
            ApiEnvelope::Error { error } => {
                return Err(format!("Playit claim/exchange error: {}", error.message()))
            }
        }
    }

    Err("Timed out waiting for Playit to return a secret key".into())
}

async fn post_envelope(
    client: &Client,
    path: &str,
    auth: Option<&str>,
    body: serde_json::Value,
) -> Result<ApiEnvelope<serde_json::Value>, String> {
    let mut request = client.post(format!("{}{}", API_BASE, path));
    if let Some(token) = auth {
        request = request.header(header::AUTHORIZATION, token);
    }

    let response = request
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Playit request to {} failed: {e}", path))?;

    let status = response.status();
    if !status.is_success() {
        let snippet = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".into());
        return Err(format!(
            "Playit request to {} returned {}: {}",
            path, status, snippet
        ));
    }

    response
        .json::<ApiEnvelope<serde_json::Value>>()
        .await
        .map_err(|e| format!("Failed to parse Playit response from {}: {e}", path))
}
