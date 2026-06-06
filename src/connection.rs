use std::collections::HashMap;

use anyhow::{anyhow, Result};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::config::Session;
use crate::ssh::{spawn_session, SessionEvent, SessionHandle};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

pub struct SessionLaunch {
    pub events: UnboundedReceiver<SessionEvent>,
    pub generation: u64,
}

pub struct SessionRuntime {
    pub session_id: String,
    pub session: Session,
    pub status: ConnectionStatus,
    pub handle: Option<SessionHandle>,
    generation: u64,
}

pub struct ConnectionManager {
    sessions: HashMap<String, SessionRuntime>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn connect(
        &mut self,
        runtime: &Handle,
        tab_id: String,
        session: Session,
        initial_cols: u32,
        initial_rows: u32,
    ) -> SessionLaunch {
        let (handle, events) = spawn_session(
            runtime,
            tab_id.clone(),
            session.clone(),
            initial_cols,
            initial_rows,
        );
        self.sessions.insert(
            tab_id.clone(),
            SessionRuntime {
                session_id: tab_id,
                session,
                status: ConnectionStatus::Connecting,
                handle: Some(handle),
                generation: 0,
            },
        );
        SessionLaunch {
            events,
            generation: 0,
        }
    }

    pub fn reconnect(
        &mut self,
        runtime: &Handle,
        tab_id: &str,
        initial_cols: u32,
        initial_rows: u32,
    ) -> Result<SessionLaunch> {
        let Some(runtime_state) = self.sessions.get_mut(tab_id) else {
            return Err(anyhow!("session runtime not found"));
        };
        if let Some(handle) = runtime_state.handle.take() {
            handle.close();
        }
        runtime_state.status = ConnectionStatus::Reconnecting;
        runtime_state.generation = runtime_state.generation.saturating_add(1);
        let generation = runtime_state.generation;
        let (handle, events) = spawn_session(
            runtime,
            tab_id.to_string(),
            runtime_state.session.clone(),
            initial_cols,
            initial_rows,
        );
        runtime_state.handle = Some(handle);
        Ok(SessionLaunch { events, generation })
    }

    pub fn disconnect(&mut self, tab_id: &str) -> bool {
        let Some(runtime_state) = self.sessions.get_mut(tab_id) else {
            return false;
        };
        if let Some(handle) = runtime_state.handle.take() {
            handle.close();
        }
        runtime_state.status = ConnectionStatus::Disconnected;
        true
    }

    pub fn remove(&mut self, tab_id: &str) {
        if let Some(mut runtime_state) = self.sessions.remove(tab_id) {
            if let Some(handle) = runtime_state.handle.take() {
                handle.close();
            }
        }
    }

    pub fn send_raw(&self, tab_id: &str, bytes: Vec<u8>) {
        if let Some(handle) = self.sessions.get(tab_id).and_then(|s| s.handle.as_ref()) {
            handle.send_raw(bytes);
        }
    }

    pub fn resize(&self, tab_id: &str, cols: u32, rows: u32) {
        if let Some(handle) = self.sessions.get(tab_id).and_then(|s| s.handle.as_ref()) {
            handle.resize(cols, rows);
        }
    }

    pub fn session(&self, tab_id: &str) -> Option<Session> {
        self.sessions.get(tab_id).map(|s| s.session.clone())
    }

    pub fn mark_connected(&mut self, tab_id: &str, generation: u64) {
        if let Some(runtime_state) = self.sessions.get_mut(tab_id) {
            if runtime_state.generation == generation {
                runtime_state.status = ConnectionStatus::Connected;
            }
        }
    }

    pub fn mark_closed(&mut self, tab_id: &str, generation: u64, reason: String) {
        if let Some(runtime_state) = self.sessions.get_mut(tab_id) {
            if runtime_state.generation == generation {
                runtime_state.handle = None;
                runtime_state.status = if reason.is_empty() {
                    ConnectionStatus::Disconnected
                } else {
                    ConnectionStatus::Failed(reason)
                };
            }
        }
    }

    pub fn is_current_generation(&self, tab_id: &str, generation: u64) -> bool {
        self.sessions
            .get(tab_id)
            .map(|s| s.session_id == tab_id && s.generation == generation)
            .unwrap_or(false)
    }
}
