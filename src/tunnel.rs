use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use directories::ProjectDirs;
use russh::client::{self, Handler};
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::load_secret_key;
use russh::{ChannelId, Disconnect};
use serde::{Deserialize, Serialize};
use ssh_key::{HashAlg, PublicKey};
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle as RuntimeHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};

use crate::config::{AuthMethod, Session};
use crate::i18n::t;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TunnelRule {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub enabled: bool,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone, Debug)]
pub enum TunnelStatus {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Failed(String),
}

impl TunnelStatus {
    pub fn text(&self) -> String {
        match self {
            TunnelStatus::Stopped => t("已停止", "Stopped").to_string(),
            TunnelStatus::Starting => t("启动中", "Starting").to_string(),
            TunnelStatus::Running => t("运行中", "Running").to_string(),
            TunnelStatus::Reconnecting => t("重连中", "Reconnecting").to_string(),
            TunnelStatus::Failed(err) => {
                format!("{}: {err}", t("失败", "Failed"))
            }
        }
    }

    pub fn kind(&self) -> i32 {
        match self {
            TunnelStatus::Stopped => 0,
            TunnelStatus::Starting => 1,
            TunnelStatus::Running => 2,
            TunnelStatus::Reconnecting => 3,
            TunnelStatus::Failed(_) => 4,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TunnelView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub local_host: String,
    pub local_port: String,
    pub remote_host: String,
    pub remote_port: String,
    pub status_text: String,
    pub status_kind: i32,
}

#[derive(Clone, Debug)]
pub enum TunnelEvent {
    Status {
        rule_id: String,
        status: TunnelStatus,
    },
    Stopped {
        rule_id: String,
    },
}

pub struct TunnelHandle {
    stop_tx: watch::Sender<bool>,
    join: JoinHandle<()>,
}

impl TunnelHandle {
    fn stop(self) {
        let _ = self.stop_tx.send(true);
        self.join.abort();
    }
}

#[derive(Default, Serialize, Deserialize)]
struct TunnelFile {
    #[serde(default)]
    rules: Vec<TunnelRule>,
}

pub struct TunnelManager {
    path: PathBuf,
    rules: Vec<TunnelRule>,
    statuses: HashMap<String, TunnelStatus>,
    handles: HashMap<String, TunnelHandle>,
    events: UnboundedSender<TunnelEvent>,
}

impl TunnelManager {
    pub fn load(events: UnboundedSender<TunnelEvent>) -> Result<Self> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir {}", parent.display()))?;
        }

        let file = if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            match serde_json::from_str::<TunnelFile>(&raw) {
                Ok(file) => file,
                Err(err) => {
                    let backup = path.with_extension("json.broken");
                    let _ = fs::rename(&path, &backup);
                    tracing::warn!(
                        "tunnel config was corrupt ({err}); backed up to {}",
                        backup.display()
                    );
                    TunnelFile::default()
                }
            }
        } else {
            TunnelFile::default()
        };

        let statuses = file
            .rules
            .iter()
            .map(|rule| (rule.id.clone(), TunnelStatus::Stopped))
            .collect();

        Ok(Self {
            path,
            rules: file.rules,
            statuses,
            handles: HashMap::new(),
            events,
        })
    }

    fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("dev", "meatshell", "meatshell")
            .context("could not determine user config directory")?;
        Ok(dirs.config_dir().join("tunnels.json"))
    }

    pub fn save(&self) -> Result<()> {
        let raw = serde_json::to_string_pretty(&TunnelFile {
            rules: self.rules.clone(),
        })?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, raw).with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("failed to finalise {}", self.path.display()))?;
        Ok(())
    }

    pub fn views_for_session(&self, session_id: &str) -> Vec<TunnelView> {
        self.rules
            .iter()
            .filter(|rule| rule.session_id == session_id)
            .map(|rule| {
                let status = self
                    .statuses
                    .get(&rule.id)
                    .cloned()
                    .unwrap_or(TunnelStatus::Stopped);
                TunnelView {
                    id: rule.id.clone(),
                    name: rule.name.clone(),
                    enabled: rule.enabled,
                    local_host: rule.local_host.clone(),
                    local_port: rule.local_port.to_string(),
                    remote_host: rule.remote_host.clone(),
                    remote_port: rule.remote_port.to_string(),
                    status_text: status.text(),
                    status_kind: status.kind(),
                }
            })
            .collect()
    }

    pub fn add_rule(&mut self, session_id: &str) -> Result<TunnelRule> {
        let local_port = self.next_local_port(session_id);
        let rule = TunnelRule {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            name: t("本地转发", "Local forward").to_string(),
            enabled: false,
            local_host: "127.0.0.1".to_string(),
            local_port,
            remote_host: "127.0.0.1".to_string(),
            remote_port: 80,
        };
        self.statuses.insert(rule.id.clone(), TunnelStatus::Stopped);
        self.rules.push(rule.clone());
        self.save()?;
        Ok(rule)
    }

    pub fn update_rule(
        &mut self,
        rule_id: &str,
        name: String,
        local_host: String,
        local_port: String,
        remote_host: String,
        remote_port: String,
    ) -> Result<Option<TunnelRule>> {
        let local_port = parse_port(&local_port, t("本地端口无效", "invalid local port"))?;
        let remote_port = parse_port(&remote_port, t("远端端口无效", "invalid remote port"))?;
        if self.handles.contains_key(rule_id) {
            self.stop_rule(rule_id);
        }
        let Some(rule) = self.rules.iter_mut().find(|rule| rule.id == rule_id) else {
            return Ok(None);
        };
        rule.name = if name.trim().is_empty() {
            t("本地转发", "Local forward").to_string()
        } else {
            name.trim().to_string()
        };
        rule.local_host = normalize_host(&local_host);
        rule.local_port = local_port;
        rule.remote_host = normalize_host(&remote_host);
        rule.remote_port = remote_port;
        let updated = rule.clone();
        self.statuses
            .insert(rule_id.to_string(), TunnelStatus::Stopped);
        self.save()?;
        Ok(Some(updated))
    }

    pub fn set_enabled(&mut self, rule_id: &str, enabled: bool) -> Result<Option<TunnelRule>> {
        if !enabled {
            self.stop_rule(rule_id);
        }
        let Some(rule) = self.rules.iter_mut().find(|rule| rule.id == rule_id) else {
            return Ok(None);
        };
        rule.enabled = enabled;
        let updated = rule.clone();
        if !enabled {
            self.statuses
                .insert(rule_id.to_string(), TunnelStatus::Stopped);
        }
        self.save()?;
        Ok(Some(updated))
    }

    pub fn delete_rule(&mut self, rule_id: &str) -> Result<()> {
        self.stop_rule(rule_id);
        self.rules.retain(|rule| rule.id != rule_id);
        self.statuses.remove(rule_id);
        self.save()
    }

    pub fn start_enabled_for_session(&mut self, runtime: &RuntimeHandle, session: Session) {
        let rules: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| rule.session_id == session.id && rule.enabled)
            .cloned()
            .collect();
        for rule in rules {
            self.start_rule(runtime, session.clone(), rule);
        }
    }

    pub fn start_rule(&mut self, runtime: &RuntimeHandle, session: Session, rule: TunnelRule) {
        if !rule.enabled || rule.session_id != session.id || self.handles.contains_key(&rule.id) {
            return;
        }
        let (stop_tx, stop_rx) = watch::channel(false);
        let events = self.events.clone();
        let rule_id = rule.id.clone();
        self.statuses
            .insert(rule_id.clone(), TunnelStatus::Starting);
        let join = runtime.spawn(async move {
            run_tunnel(session, rule, stop_rx, events).await;
        });
        self.handles.insert(rule_id, TunnelHandle { stop_tx, join });
    }

    pub fn stop_for_session(&mut self, session_id: &str) {
        let ids: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| rule.session_id == session_id)
            .map(|rule| rule.id.clone())
            .collect();
        for id in ids {
            self.stop_rule(&id);
        }
    }

    pub fn apply_event(&mut self, event: &TunnelEvent) {
        match event {
            TunnelEvent::Status { rule_id, status } => {
                self.statuses.insert(rule_id.clone(), status.clone());
            }
            TunnelEvent::Stopped { rule_id } => {
                self.statuses.insert(rule_id.clone(), TunnelStatus::Stopped);
                self.handles.remove(rule_id);
            }
        }
    }

    fn stop_rule(&mut self, rule_id: &str) {
        if let Some(handle) = self.handles.remove(rule_id) {
            handle.stop();
        }
        self.statuses
            .insert(rule_id.to_string(), TunnelStatus::Stopped);
    }

    fn next_local_port(&self, session_id: &str) -> u16 {
        let mut port = 8080;
        while self
            .rules
            .iter()
            .any(|rule| rule.session_id == session_id && rule.local_port == port)
        {
            port = port.saturating_add(1);
        }
        port
    }
}

async fn run_tunnel(
    session: Session,
    rule: TunnelRule,
    mut stop_rx: watch::Receiver<bool>,
    events: UnboundedSender<TunnelEvent>,
) {
    let mut retry = 0u32;
    loop {
        if *stop_rx.borrow() {
            break;
        }
        let status = if retry == 0 {
            TunnelStatus::Starting
        } else {
            TunnelStatus::Reconnecting
        };
        send_status(&events, &rule.id, status);

        match connect_and_serve(&session, &rule, &mut stop_rx, &events).await {
            Ok(ServeExit::Stopped) => break,
            Err(err) => {
                send_status(&events, &rule.id, TunnelStatus::Failed(format!("{err:#}")));
                retry = retry.saturating_add(1);
                let delay = backoff_delay(retry);
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        }
    }
    let _ = events.send(TunnelEvent::Stopped { rule_id: rule.id });
}

enum ServeExit {
    Stopped,
}

async fn connect_and_serve(
    session: &Session,
    rule: &TunnelRule,
    stop_rx: &mut watch::Receiver<bool>,
    events: &UnboundedSender<TunnelEvent>,
) -> Result<ServeExit> {
    let handle = connect_ssh(session).await?;
    let bind_addr = format!("{}:{}", rule.local_host, rule.local_port);
    let listener = TcpListener::bind((rule.local_host.as_str(), rule.local_port))
        .await
        .with_context(|| format!("listen {bind_addr}"))?;
    send_status(events, &rule.id, TunnelStatus::Running);
    let mut forwards = JoinSet::new();

    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    forwards.abort_all();
                    while forwards.join_next().await.is_some() {}
                    return Ok(ServeExit::Stopped);
                }
            }
            _ = forwards.join_next(), if !forwards.is_empty() => {}
            accepted = listener.accept() => {
                let (local, peer) = accepted.with_context(|| format!("accept {bind_addr}"))?;
                let origin_host = peer.ip().to_string();
                let origin_port = u32::from(peer.port());
                let channel = handle
                    .channel_open_direct_tcpip(
                        rule.remote_host.clone(),
                        u32::from(rule.remote_port),
                        origin_host,
                        origin_port,
                    )
                    .await
                    .with_context(|| {
                        format!(
                            "open direct-tcpip {}:{}",
                            rule.remote_host, rule.remote_port
                        )
                    })?;
                forwards.spawn(forward_streams(local, channel.into_stream()));
            }
        }
    }
}

async fn forward_streams<S>(mut local: TcpStream, mut remote: S)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let _ = copy_bidirectional(&mut local, &mut remote).await;
}

async fn connect_ssh(session: &Session) -> Result<client::Handle<TunnelClientHandler>> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(60 * 30)),
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..<_>::default()
    });
    let addr = format!("{}:{}", session.host, session.port);
    let mut handle = match crate::proxy::resolve(&session.proxy) {
        Some(proxy) => {
            let stream = crate::proxy::connect(&proxy, &session.host, session.port)
                .await
                .with_context(|| format!("tunnel proxy connect {} failed", addr))?;
            client::connect_stream(config, stream, TunnelClientHandler)
                .await
                .with_context(|| format!("tunnel connect {} failed", addr))?
        }
        None => client::connect(config, addr.as_str(), TunnelClientHandler)
            .await
            .with_context(|| format!("tunnel connect {} failed", addr))?,
    };

    authenticate(&mut handle, session).await?;
    Ok(handle)
}

async fn authenticate(
    handle: &mut client::Handle<TunnelClientHandler>,
    session: &Session,
) -> Result<()> {
    let authed = match session.auth {
        AuthMethod::Password => handle
            .authenticate_password(&session.user, session.password.as_str())
            .await
            .context("tunnel password auth failed")?,
        AuthMethod::Key => {
            let raw = session.private_key_path.trim();
            if raw.is_empty() {
                return Err(anyhow!(t("私钥路径为空", "private key path is empty")));
            }
            let normalised = raw.replace('\\', "/");
            let key_path = normalised
                .strip_suffix(".pub")
                .map(str::to_string)
                .unwrap_or(normalised);
            let keypair = load_secret_key(Path::new(&key_path), None)
                .with_context(|| format!("failed to load key {key_path}"))?;
            let hash = if keypair.algorithm().is_rsa() {
                Some(HashAlg::Sha256)
            } else {
                None
            };
            let key_with_hash = PrivateKeyWithHashAlg::new(Arc::new(keypair), hash)
                .context("invalid private key")?;
            handle
                .authenticate_publickey(&session.user, key_with_hash)
                .await
                .context("tunnel publickey auth failed")?
        }
    };

    if authed {
        Ok(())
    } else {
        let _ = handle
            .disconnect(Disconnect::ByApplication, "auth failed", "")
            .await;
        Err(anyhow!(t("隧道认证失败", "tunnel authentication failed")))
    }
}

fn parse_port(value: &str, label: &str) -> Result<u16> {
    let port: u16 = value.trim().parse().with_context(|| label.to_string())?;
    if port == 0 {
        Err(anyhow!(label.to_string()))
    } else {
        Ok(port)
    }
}

fn normalize_host(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "127.0.0.1".to_string()
    } else {
        trimmed.to_string()
    }
}

fn backoff_delay(retry: u32) -> Duration {
    let secs = match retry {
        0 | 1 => 1,
        2 => 2,
        3 => 5,
        _ => 10,
    };
    Duration::from_secs(secs)
}

fn send_status(events: &UnboundedSender<TunnelEvent>, rule_id: &str, status: TunnelStatus) {
    let _ = events.send(TunnelEvent::Status {
        rule_id: rule_id.to_string(),
        status,
    });
}

struct TunnelClientHandler;

#[async_trait]
impl Handler for TunnelClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        _data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
