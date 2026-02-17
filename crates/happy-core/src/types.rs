//! Core type definitions for Happy Coding

use serde::{Deserialize, Serialize};

/// Supported AI coding platforms
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Claude,
    Codex,
    Antigravity,
}

impl Platform {
    /// Get all available platforms
    pub fn all() -> &'static [Platform] {
        &[Platform::Claude, Platform::Codex, Platform::Antigravity]
    }

    /// Get the platform name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Claude => "claude",
            Platform::Codex => "codex",
            Platform::Antigravity => "antigravity",
        }
    }

    /// Get the default output directory for this platform
    pub fn default_output_dir(&self) -> &'static str {
        match self {
            Platform::Claude => ".claude",
            Platform::Codex => ".codex",
            Platform::Antigravity => ".agent",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Supported features across platforms
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Feature {
    Skill,
    Workflow,
    Command,
    Hooks,
    Mcp,
    Rules,
}

impl Feature {
    pub fn as_str(&self) -> &'static str {
        match self {
            Feature::Skill => "skill",
            Feature::Workflow => "workflow",
            Feature::Command => "command",
            Feature::Hooks => "hooks",
            Feature::Mcp => "mcp",
            Feature::Rules => "rules",
        }
    }
}

impl std::fmt::Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Skill parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub param_type: ParameterType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Array,
}

/// Skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<SkillParameter>,
    #[serde(default)]
    pub examples: Vec<SkillExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExample {
    pub input: String,
    pub output: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Workflow step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
}

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub triggers: Vec<String>,
}

/// Command definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub workflow: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Target configuration for a platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub output_dir: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            output_dir: None,
        }
    }
}

/// All target configurations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetsConfig {
    #[serde(default)]
    pub claude: Option<TargetConfig>,
    #[serde(default)]
    pub codex: Option<TargetConfig>,
    #[serde(default)]
    pub antigravity: Option<TargetConfig>,
}

impl TargetsConfig {
    /// Get target config for a specific platform
    pub fn get(&self, platform: Platform) -> Option<&TargetConfig> {
        match platform {
            Platform::Claude => self.claude.as_ref(),
            Platform::Codex => self.codex.as_ref(),
            Platform::Antigravity => self.antigravity.as_ref(),
        }
    }

    /// Get enabled platforms
    pub fn enabled_platforms(&self) -> Vec<Platform> {
        let mut platforms = Vec::new();
        if self.claude.as_ref().map(|t| t.enabled).unwrap_or(false) {
            platforms.push(Platform::Claude);
        }
        if self.codex.as_ref().map(|t| t.enabled).unwrap_or(false) {
            platforms.push(Platform::Codex);
        }
        if self
            .antigravity
            .as_ref()
            .map(|t| t.enabled)
            .unwrap_or(false)
        {
            platforms.push(Platform::Antigravity);
        }

        platforms
    }
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Http,
}

/// MCP configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

/// Main project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub targets: TargetsConfig,
    #[serde(default)]
    pub skills: Vec<SkillDefinition>,
    #[serde(default)]
    pub workflows: Vec<WorkflowDefinition>,
    #[serde(default)]
    pub commands: Vec<CommandDefinition>,
    #[serde(default)]
    pub mcp: Option<McpConfig>,
}

impl ProjectConfig {
    /// Create a new default project configuration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            targets: TargetsConfig {
                claude: Some(TargetConfig::default()),
                codex: Some(TargetConfig::default()),
                antigravity: None,
            },
            skills: Vec::new(),
            workflows: Vec::new(),
            commands: Vec::new(),
            mcp: None,
        }
    }

    /// Get the output directory for a platform
    pub fn output_dir(&self, platform: Platform) -> String {
        self.targets
            .get(platform)
            .and_then(|t| t.output_dir.clone())
            .unwrap_or_else(|| platform.default_output_dir().to_string())
    }
}

/// Build result for a single platform
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
    pub platform: Platform,
    pub output_path: String,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl BuildResult {
    /// Create a successful build result
    pub fn success(platform: Platform, output_path: impl Into<String>, files: Vec<String>) -> Self {
        Self {
            success: true,
            platform,
            output_path: output_path.into(),
            files,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Create a failed build result
    pub fn failure(platform: Platform, errors: Vec<String>) -> Self {
        Self {
            success: false,
            platform,
            output_path: String::new(),
            files: Vec::new(),
            warnings: Vec::new(),
            errors,
        }
    }
}

/// Validation result
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_error(mut self, error: ValidationError) -> Self {
        self.valid = false;
        self.errors.push(error);
        self
    }

    pub fn with_warning(mut self, warning: ValidationWarning) -> Self {
        self.warnings.push(warning);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Install target specification
#[derive(Debug, Clone)]
pub struct InstallTarget {
    pub platform: Platform,
    pub global: bool,
    pub project_path: Option<String>,
}

/// Build options
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    pub target: Option<Platform>,
    pub watch: bool,
    pub clean: bool,
}

/// Build summary across all platforms
#[derive(Debug, Clone)]
pub struct BuildSummary {
    pub success: bool,
    pub results: Vec<BuildResult>,
    pub duration_ms: u64,
    pub platforms: Vec<Platform>,
}

// =============== Remote Control Types ===============

use chrono::{DateTime, Utc};

/// User account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegistration {
    pub email: String,
    pub name: Option<String>,
    pub password: String,
}

/// User login request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLogin {
    pub email: String,
    pub password: String,
}

/// Authentication tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

/// AI provider types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AIProvider {
    Anthropic,
    OpenAI,
    Azure,
    Gemini,
}

/// AI profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProfile {
    pub name: String,
    pub provider: AIProvider,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub env_vars: std::collections::HashMap<String, String>,
}

/// Registered machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub public_key: Vec<u8>,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// User settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub version: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub server_url: String,
    pub webapp_url: String,
    pub profiles: Vec<AIProfile>,
    pub active_profile: Option<String>,
    #[serde(default)]
    pub machines: Vec<Machine>,
    /// Unique machine identifier (loaded from separate file, not synced in settings.json)
    #[serde(skip)]
    pub machine_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            user_id: None,
            email: None,
            password: None,
            access_token: None,
            refresh_token: None,
            server_url: "https://api.happy-remote.dev".to_string(),
            webapp_url: "https://app.happy-remote.dev".to_string(),
            profiles: Vec::new(),
            active_profile: None,
            machines: Vec::new(),
            machine_id: String::new(),
        }
    }
}
