//! Session / application configuration.
//!
//! Persists a simple JSON file under the platform's standard config dir
//! (e.g. `%APPDATA%/meatshell/sessions.json` on Windows).
//!
//! The password field is stored in plain text for v0.1; a proper OS keychain
//! integration is tracked for a later iteration.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::terminal::engine::TerminalEngineMode;

/// A secret string (e.g. a session password) whose heap buffer is zeroed when
/// it is dropped, so plaintext credentials don't survive in freed memory and
/// turn up in core dumps, a debugger, or `/proc/<pid>/mem`.  `Clone` makes an
/// independent copy that is likewise zeroed on its own drop, and `Debug` is
/// redacted so a password can never be logged by accident.
#[derive(Clone, Default)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never reveal the contents in logs / debug output.
        f.write_str(if self.0.is_empty() {
            "Secret(\"\")"
        } else {
            "Secret(***)"
        })
    }
}

impl Serialize for Secret {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Secret(String::deserialize(d)?))
    }
}

/// How a session authenticates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    Password,
    Key,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthMethod::Password => "password",
            AuthMethod::Key => "key",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "key" => AuthMethod::Key,
            _ => AuthMethod::Password,
        }
    }
}

/// A single saved SSH target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthMethod,
    #[serde(default)]
    pub password: Secret,
    #[serde(default)]
    pub private_key_path: String,
    /// Optional outbound proxy, e.g. "socks5://127.0.0.1:1080" or
    /// "http://user:pass@host:8080". Empty = use $ALL_PROXY, else direct.
    #[serde(default)]
    pub proxy: String,
    /// Optional folder/group name to organize sessions in the list.
    /// Empty = ungrouped. Sessions are grouped by this in Quick Connect.
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub last_used: Option<String>,
}

impl Session {
    pub fn new_empty() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            host: String::new(),
            port: 22,
            user: "root".into(),
            auth: AuthMethod::Password,
            password: Secret::default(),
            private_key_path: String::new(),
            proxy: String::new(),
            group: String::new(),
            last_used: None,
        }
    }
}

/// On-disk layout. Keep additive to ease forward-compat.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    pub sessions: Vec<Session>,
    /// Preset SFTP download directory. Empty = ask each time.
    #[serde(default)]
    pub download_dir: String,
    /// UI language code: "zh" (default) or "en".
    #[serde(default)]
    pub language: String,
    /// Theme preference: "system" (default) | "dark" | "light".
    #[serde(default)]
    pub theme_pref: String,
    /// Terminal font family. Empty = the built-in default (Cascadia Mono).
    #[serde(default)]
    pub font_family: String,
    /// Terminal font size in px. 0 = the built-in default.
    #[serde(default)]
    pub font_size: u32,
    #[serde(default)]
    pub terminal_engine: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportFile {
    meatshell_export: u32,
    sessions: Vec<Session>,
}

pub struct ConfigStore {
    path: PathBuf,
    cache: ConfigFile,
}

impl ConfigStore {
    /// Load (or initialise) the config file. On any parse error we back up the
    /// broken file and start fresh — losing saved sessions is better than
    /// crashing at launch.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir {}", parent.display()))?;
        }

        let existed = path.exists();
        let mut cache = if existed {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            match serde_json::from_str::<ConfigFile>(&raw) {
                Ok(cfg) => cfg,
                Err(err) => {
                    let backup = path.with_extension("json.broken");
                    let _ = fs::rename(&path, &backup);
                    tracing::warn!(
                        "config file was corrupt ({err}); backed up to {}",
                        backup.display()
                    );
                    ConfigFile::default()
                }
            }
        } else {
            ConfigFile::default()
        };

        let normalized_language = crate::i18n::normalize_language(&cache.language).to_string();
        let language_changed = cache.language != normalized_language;
        cache.language = normalized_language;

        let store = Self { path, cache };
        if existed && language_changed {
            if let Err(err) = store.save() {
                tracing::warn!("failed to save normalized language config: {err:#}");
            }
        }

        Ok(store)
    }

    fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("dev", "meatshell", "meatshell")
            .context("could not determine user config directory")?;
        Ok(dirs.config_dir().join("sessions.json"))
    }

    pub fn sessions(&self) -> &[Session] {
        &self.cache.sessions
    }

    #[allow(dead_code)] // reserved for an upcoming reorder/drag-drop feature
    pub fn sessions_mut(&mut self) -> &mut Vec<Session> {
        &mut self.cache.sessions
    }

    pub fn upsert(&mut self, session: Session) {
        if let Some(existing) = self.cache.sessions.iter_mut().find(|s| s.id == session.id) {
            *existing = session;
        } else {
            self.cache.sessions.push(session);
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.cache.sessions.retain(|s| s.id != id);
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.cache.sessions.iter().find(|s| s.id == id)
    }

    pub fn download_dir(&self) -> &str {
        &self.cache.download_dir
    }

    pub fn set_download_dir(&mut self, dir: String) {
        self.cache.download_dir = dir;
    }

    /// UI language code ("zh" default / "en").
    pub fn language(&self) -> &'static str {
        crate::i18n::normalize_language(&self.cache.language)
    }

    pub fn set_language(&mut self, lang: String) {
        self.cache.language = crate::i18n::normalize_language(&lang).to_string();
    }

    /// Theme preference: "system" (default) | "dark" | "light".
    pub fn theme_pref(&self) -> &str {
        if self.cache.theme_pref.is_empty() {
            "system"
        } else {
            &self.cache.theme_pref
        }
    }

    pub fn set_theme_pref(&mut self, pref: String) {
        self.cache.theme_pref = pref;
    }

    /// Terminal font family ("" = built-in default).
    pub fn font_family(&self) -> &str {
        &self.cache.font_family
    }

    pub fn set_font_family(&mut self, family: String) {
        self.cache.font_family = family;
    }

    /// Terminal font size in px (falls back to 13 when unset).
    pub fn font_size(&self) -> u32 {
        if self.cache.font_size == 0 {
            13
        } else {
            self.cache.font_size
        }
    }

    pub fn set_font_size(&mut self, size: u32) {
        self.cache.font_size = size.clamp(8, 32);
    }

    pub fn terminal_engine_mode(&self) -> TerminalEngineMode {
        self.cache
            .terminal_engine
            .as_deref()
            .map(TerminalEngineMode::from_str)
            .unwrap_or(TerminalEngineMode::Alacritty)
    }

    pub fn set_terminal_engine_mode(&mut self, mode: TerminalEngineMode) {
        self.cache.terminal_engine = Some(mode.as_str().to_string());
    }

    pub fn save(&self) -> Result<()> {
        let raw = serde_json::to_string_pretty(&self.cache)?;
        // Write to a sibling temp file then rename — cheap atomicity on most
        // platforms. Good enough for a config file.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, raw).with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("failed to finalise {}", self.path.display()))?;
        Ok(())
    }

    pub fn export_to(&self, path: &Path) -> Result<usize> {
        let mut out = ExportFile {
            meatshell_export: 1,
            sessions: self.cache.sessions.clone(),
        };
        for session in &mut out.sessions {
            session.last_used = None;
        }
        let raw = serde_json::to_string_pretty(&out)?;
        fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(out.sessions.len())
    }

    pub fn import_from(&mut self, path: &Path) -> Result<(usize, usize)> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file: ExportFile =
            serde_json::from_str(&raw).context("not a valid meatshell export file")?;

        let mut added = 0usize;
        let mut skipped = 0usize;
        for mut session in file.sessions {
            let duplicate = self.cache.sessions.iter().any(|existing| {
                existing.host == session.host
                    && existing.user == session.user
                    && existing.port == session.port
            });
            if duplicate {
                skipped += 1;
                continue;
            }
            session.id = Uuid::new_v4().to_string();
            session.last_used = None;
            self.cache.sessions.push(session);
            added += 1;
        }
        if added > 0 {
            self.save()?;
        }
        Ok((added, skipped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_engine_defaults_to_alacritty_when_unset() {
        let store = ConfigStore {
            path: PathBuf::new(),
            cache: ConfigFile::default(),
        };
        assert_eq!(store.terminal_engine_mode(), TerminalEngineMode::Alacritty);
    }

    #[test]
    fn terminal_engine_uses_saved_value_when_present() {
        let store = ConfigStore {
            path: PathBuf::new(),
            cache: ConfigFile {
                terminal_engine: Some("legacy".to_string()),
                ..ConfigFile::default()
            },
        };
        assert_eq!(store.terminal_engine_mode(), TerminalEngineMode::Legacy);
    }

    #[test]
    fn legacy_config_defaults_new_fields() {
        let raw = r#"{
            "sessions": [{
                "id": "1",
                "name": "prod",
                "host": "example.com",
                "port": 22,
                "user": "root",
                "auth": "password"
            }]
        }"#;
        let cfg: ConfigFile = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.theme_pref, "");
        assert_eq!(cfg.font_family, "");
        assert_eq!(cfg.font_size, 0);
        assert_eq!(cfg.sessions[0].group, "");
    }

    #[test]
    fn language_values_are_normalized() {
        let mut store = ConfigStore {
            path: PathBuf::new(),
            cache: ConfigFile {
                language: "ja".into(),
                ..ConfigFile::default()
            },
        };
        assert_eq!(store.language(), "zh");

        store.set_language("en-US".into());
        assert_eq!(store.cache.language, "en");

        store.set_language("ja".into());
        assert_eq!(store.cache.language, "zh");
    }

    #[test]
    fn export_import_roundtrip_skips_duplicates() {
        let export_path = std::env::temp_dir().join(format!("ms-exp-{}.json", Uuid::new_v4()));
        let mut source = ConfigStore {
            path: PathBuf::new(),
            cache: ConfigFile::default(),
        };
        source.cache.sessions.push(Session {
            name: "prod".into(),
            host: "example.com".into(),
            user: "root".into(),
            group: "Servers".into(),
            password: Secret::new("secret"),
            ..Session::new_empty()
        });

        assert_eq!(source.export_to(&export_path).unwrap(), 1);

        let target_path = std::env::temp_dir().join(format!("ms-target-{}.json", Uuid::new_v4()));
        let mut target = ConfigStore {
            path: target_path,
            cache: ConfigFile::default(),
        };
        assert_eq!(target.import_from(&export_path).unwrap(), (1, 0));
        assert_eq!(target.cache.sessions[0].host, "example.com");
        assert_eq!(target.cache.sessions[0].group, "Servers");
        assert_eq!(target.import_from(&export_path).unwrap(), (0, 1));

        let _ = std::fs::remove_file(&export_path);
        let _ = std::fs::remove_file(&target.path);
    }
}
