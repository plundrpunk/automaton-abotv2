use serde::Deserialize;
use std::path::PathBuf;

/// Root configuration for Abot v3.
/// Loaded from config/abot.toml with env var overrides.
#[derive(Debug, Deserialize, Clone)]
pub struct AbotConfig {
    pub agent: AgentConfig,
    pub ams: AmsConfig,
    pub llm: LlmConfig,
    pub sandbox: SandboxConfig,
    pub security: SecurityConfig,
    pub channels: ChannelsConfig,
    pub mcp: McpConfig,
    pub telemetry: TelemetryConfig,
    pub hands: HandsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    pub name: String,
    #[serde(default = "default_agent_id")]
    pub id: String,
}

fn default_agent_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct AmsConfig {
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u64,
}

fn default_heartbeat_interval() -> u64 { 10 }
fn default_connect_timeout() -> u64 { 5000 }
fn default_request_timeout() -> u64 { 30000 }

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub kilo: Option<KiloConfig>,
    pub routing: Option<LlmRouting>,
    pub fallback: Option<LlmFallback>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Kilo,
    Direct,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KiloConfig {
    #[serde(default = "default_kilo_binary")]
    pub binary: String,
    #[serde(default = "default_kilo_mode")]
    pub default_mode: String,
}

fn default_kilo_binary() -> String { "kilo".into() }
fn default_kilo_mode() -> String { "code".into() }

#[derive(Debug, Deserialize, Clone)]
pub struct LlmRouting {
    pub simple_chat: Option<String>,
    pub code_generation: Option<String>,
    pub architecture: Option<String>,
    pub debugging: Option<String>,
    pub research: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmFallback {
    pub chain: Vec<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_max_retries() -> u32 { 2 }

#[derive(Debug, Deserialize, Clone)]
pub struct SandboxConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_engine")]
    pub engine: String,
    #[serde(default = "default_fuel_limit")]
    pub fuel_limit: u64,
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u32,
    #[serde(default = "default_epoch_deadline")]
    pub epoch_deadline_ms: u64,
    #[serde(default)]
    pub allowed_paths: Vec<PathBuf>,
    #[serde(default)]
    pub network_allowed: bool,
}

fn default_true() -> bool { true }
fn default_engine() -> String { "wasmtime".into() }
fn default_fuel_limit() -> u64 { 10_000_000_000 }
fn default_memory_limit() -> u32 { 256 }
fn default_epoch_deadline() -> u64 { 60_000 }

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    #[serde(default = "default_true")]
    pub merkle_audit: bool,
    #[serde(default = "default_true")]
    pub secret_zeroization: bool,
    #[serde(default = "default_true")]
    pub taint_tracking: bool,
    #[serde(default = "default_true")]
    pub ssrf_protection: bool,
    #[serde(default = "default_true")]
    pub manifest_signing: bool,
    #[serde(default)]
    pub signing_key_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ChannelsConfig {
    pub telegram: Option<ChannelAdapterConfig>,
    pub discord: Option<ChannelAdapterConfig>,
    pub slack: Option<ChannelAdapterConfig>,
    pub whatsapp: Option<ChannelAdapterConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChannelAdapterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpConfig {
    #[serde(default)]
    pub server_enabled: bool,
    #[serde(default = "default_mcp_port")]
    pub server_port: u16,
    #[serde(default)]
    pub clients: Vec<McpClientConfig>,
}

fn default_mcp_port() -> u16 { 5100 }

#[derive(Debug, Deserialize, Clone)]
pub struct McpClientConfig {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelemetryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_log_format")]
    pub log_format: String,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

fn default_log_level() -> String { "info".into() }
fn default_log_format() -> String { "json".into() }
fn default_metrics_port() -> u16 { 9090 }

#[derive(Debug, Deserialize, Clone)]
pub struct HandsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_hands_dir")]
    pub directory: PathBuf,
}

fn default_hands_dir() -> PathBuf { PathBuf::from("./hands") }

impl AbotConfig {
    /// Load config from TOML file with environment variable overrides.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let mut config: AbotConfig = toml::from_str(&contents)?;

        // Env var overrides (secrets should never be in TOML)
        if let Ok(key) = std::env::var("AUTOMATON_AMS_API_KEY") {
            config.ams.api_key = key;
        }
        if let Ok(url) = std::env::var("AUTOMATON_AMS_URL") {
            config.ams.url = url;
        }
        if let Ok(id) = std::env::var("AUTOMATON_AGENT_ID") {
            config.agent.id = id;
        }
        if let Ok(name) = std::env::var("AUTOMATON_AGENT_NAME") {
            config.agent.name = name;
        }

        Ok(config)
    }
}
