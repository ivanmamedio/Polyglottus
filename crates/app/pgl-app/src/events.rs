//! Eventos e progresso para a camada de aplicação

use super::{JobProgress, JobState, QaWarning, UnitId, Uuid};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Evento de progresso de job para UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JobEvent {
    Started { job_id: Uuid },
    Progress { job_id: Uuid, progress: JobProgress },
    Paused { job_id: Uuid },
    Resumed { job_id: Uuid },
    Completed { job_id: Uuid },
    Failed { job_id: Uuid, error: String },
    Cancelled { job_id: Uuid },
}

/// Evento de unidade
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UnitEvent {
    Translated { job_id: Uuid, unit_id: UnitId },
    Flagged { job_id: Uuid, unit_id: UnitId, warnings: Vec<QaWarning> },
    Failed { job_id: Uuid, unit_id: UnitId, error: String },
    Reviewed { job_id: Uuid, unit_id: UnitId },
    Retranslating { job_id: Uuid, unit_id: UnitId },
}

/// Evento de modelo
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModelEvent {
    Downloading { model_id: String, downloaded: u64, total: u64 },
    Downloaded { model_id: String },
    LoadError { model_id: String, error: String },
    Unloaded { model_id: String },
}

/// Evento de log estruturado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub timestamp: String, // ISO 8601
    pub job_id: Option<Uuid>,
    pub unit_id: Option<UnitId>,
}

/// Nível de log
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Agregador de eventos para UI (streaming)
pub struct EventStream {
    // Em implementação real: broadcast channel, WebSocket, SSE, etc.
}

impl EventStream {
    pub fn new() -> Self {
        Self {}
    }

    pub fn subscribe_job(&self, job_id: Uuid) -> JobEventStream {
        JobEventStream { job_id }
    }

    pub fn subscribe_units(&self, job_id: Uuid) -> UnitEventStream {
        UnitEventStream { job_id }
    }
}

/// Stream de eventos de job
pub struct JobEventStream {
    job_id: Uuid,
}

impl JobEventStream {
    pub async fn next(&mut self) -> Option<JobEvent> {
        // Implementação real: receiver de channel
        None
    }
}

/// Stream de eventos de unidade
pub struct UnitEventStream {
    job_id: Uuid,
}

impl UnitEventStream {
    pub async fn next(&mut self) -> Option<UnitEvent> {
        None
    }
}

/// Progresso formatado para exibição
pub fn format_progress(progress: &JobProgress) -> String {
    let eta_str = progress.eta
        .map(|d| format_eta(d))
        .unwrap_or_else(|| "calculando...".into());

    format!(
        "[{:>3}%] {}/{} | {:.1} un/s | ETA: {}",
        progress.percent,
        progress.completed,
        progress.total,
        progress.units_per_second,
        eta_str
    )
}

/// Formata duração como ETA
pub fn format_eta(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Estado do job para UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobUiState {
    pub id: Uuid,
    pub name: String,
    pub target_language: String,
    pub state: JobState,
    pub progress: JobProgress,
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_cancel: bool,
    pub can_retry: bool,
}

impl JobUiState {
    pub fn from_job(job: &super::Job) -> Self {
        Self {
            id: job.id,
            name: job.config.lang_pair.target.0.clone(),
            target_language: job.config.lang_pair.target.0.clone(),
            state: job.state.clone(),
            progress: job.progress.clone(),
            can_pause: job.state == JobState::Running,
            can_resume: job.state == JobState::Paused || job.state == JobState::Created,
            can_cancel: matches!(job.state, JobState::Running | JobState::Paused),
            can_retry: matches!(job.state, JobState::Failed(_)),
        }
    }
}