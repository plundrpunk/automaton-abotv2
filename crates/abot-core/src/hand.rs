//! HAND.toml manifest loader.
//!
//! Each shipped body has a `hands/<name>/` directory containing:
//! - `HAND.toml`       - identity, persona, runtime hints, tool permissions
//! - `system_prompt.md` - runtime-facing prompt text
//! - `SKILL.md`        - role card
//!
//! The runtime resolves the hand directory from the agent name and the
//! `[hands].directory` config path. If a matching directory exists and
//! contains a valid HAND.toml, the manifest is loaded and its metadata
//! is sent to AMS during birth ritual.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Parsed HAND.toml manifest for a shipped body.
#[derive(Debug, Clone, Deserialize)]
pub struct HandManifest {
    #[serde(default)]
    pub schema_version: String,
    pub hand: HandIdentity,
    #[serde(default)]
    pub matching: Option<HandMatching>,
    #[serde(default)]
    pub runtime: Option<HandRuntime>,
    #[serde(default)]
    pub persona: Option<HandPersona>,
    #[serde(default)]
    pub tool_permissions: Vec<String>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HandIdentity {
    pub name: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub persona: String,
    #[serde(default)]
    pub archetype: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub primary_use: String,
    #[serde(default)]
    pub skill_file: String,
    #[serde(default)]
    pub system_prompt_file: String,
    #[serde(default)]
    pub default_model: String,
    #[serde(default)]
    pub launcher_group: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HandMatching {
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub requires_seeded_head: bool,
    #[serde(default)]
    pub seed_name: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HandRuntime {
    #[serde(default)]
    pub agent_class: String,
    #[serde(default)]
    pub trust_tier: u8,
    #[serde(default)]
    pub enable_tools: bool,
    #[serde(default)]
    pub max_iterations: u32,
    #[serde(default)]
    pub operating_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HandPersona {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub focus: String,
}

/// A loaded hand with its manifest and resolved file paths.
#[derive(Debug, Clone)]
pub struct LoadedHand {
    pub manifest: HandManifest,
    pub dir: PathBuf,
    pub system_prompt: Option<String>,
}

impl LoadedHand {
    /// Get identity claims suitable for sending to AMS in birth/heartbeat.
    ///
    /// SECURITY: This only sends identity and persona claims — descriptive
    /// metadata about who the body thinks it is. It deliberately DOES NOT
    /// send trust_tier, agent_class, tool_permissions, max_iterations, or
    /// enable_tools. Those are AMS-granted fields that come from the seeded
    /// agent database. The body is open-source; it cannot self-assert its
    /// own privilege level.
    ///
    /// Claims vs Grants:
    ///   CLAIMS (body sends):  hand_name, archetype, domain, description,
    ///                         persona_role, persona_style, default_model,
    ///                         goals, tags
    ///   GRANTS (AMS returns): trust_tier, agent_class, tool_permissions,
    ///                         max_iterations, enable_tools, thresholds
    pub fn to_ams_claims(&self) -> serde_json::Value {
        let hand = &self.manifest.hand;
        let mut meta = serde_json::json!({
            "hand_name": hand.name,
            "archetype": hand.archetype,
            "domain": hand.domain,
            "description": hand.description,
            "default_model": hand.default_model,
        });

        if let Some(persona) = &self.manifest.persona {
            meta["persona_role"] = serde_json::json!(persona.role);
            meta["persona_style"] = serde_json::json!(persona.style);
        }

        if !self.manifest.goals.is_empty() {
            meta["goals"] = serde_json::json!(self.manifest.goals);
        }

        if !self.manifest.tags.is_empty() {
            meta["tags"] = serde_json::json!(self.manifest.tags);
        }

        meta
    }
}

/// Try to load the hand manifest for a given agent name.
///
/// Resolution order:
/// 1. `AUTOMATON_HAND_DIR` env var (set by run_hands.py)
/// 2. `<hands_dir>/<agent_name>/HAND.toml`
///
/// Returns `None` if no hand directory or manifest is found (not an error —
/// the runtime can work without a hand manifest).
pub fn load_hand(hands_dir: &Path, agent_name: &str) -> Option<LoadedHand> {
    // Check env var override first (set by launcher script)
    let hand_dir = if let Ok(dir) = std::env::var("AUTOMATON_HAND_DIR") {
        PathBuf::from(dir)
    } else {
        hands_dir.join(agent_name)
    };

    let toml_path = hand_dir.join("HAND.toml");
    if !toml_path.exists() {
        debug!(agent = agent_name, path = ?toml_path, "No HAND.toml found, running without hand manifest");
        return None;
    }

    match load_manifest(&toml_path) {
        Ok(manifest) => {
            // Load system prompt if referenced
            let system_prompt = load_system_prompt(&hand_dir, &manifest);

            info!(
                hand = %manifest.hand.name,
                archetype = %manifest.hand.archetype,
                domain = %manifest.hand.domain,
                has_system_prompt = system_prompt.is_some(),
                "Hand manifest loaded"
            );

            Some(LoadedHand {
                manifest,
                dir: hand_dir,
                system_prompt,
            })
        }
        Err(e) => {
            warn!(
                agent = agent_name,
                error = %e,
                "Failed to load HAND.toml, running without hand manifest"
            );
            None
        }
    }
}

fn load_manifest(path: &Path) -> Result<HandManifest> {
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: HandManifest =
        toml::from_str(&contents).with_context(|| format!("parsing {}", path.display()))?;
    Ok(manifest)
}

fn load_system_prompt(hand_dir: &Path, manifest: &HandManifest) -> Option<String> {
    let filename = if manifest.hand.system_prompt_file.is_empty() {
        "system_prompt.md"
    } else {
        &manifest.hand.system_prompt_file
    };

    let path = hand_dir.join(filename);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            debug!(path = ?path, len = content.len(), "System prompt loaded");
            Some(content)
        }
        Err(_) => {
            debug!(path = ?path, "No system prompt file found");
            None
        }
    }
}
