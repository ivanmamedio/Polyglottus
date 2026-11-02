//! Job de tradução - estado, configuração e progresso

use super::{ExportOptions, FidelityReport, LangPair, SectionId, Unit, UnitId, UnitState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Job de tradução completo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub config: JobConfig,
    pub state: JobState,
    pub progress: JobProgress,
    pub units: HashMap<UnitId, Unit>,
    pub sections: Vec<SectionId>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub fidelity_report: Option<FidelityReport>,
    pub export_options: Option<ExportOptions>,
    pub export_report: Option<super::ExportReport>,
}

impl Job {
    /// Cria novo job
    pub fn new(config: JobConfig) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::now_v7(),
            config,
            state: JobState::Created,
            progress: JobProgress::default(),
            units: HashMap::new(),
            sections: Vec::new(),
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            fidelity_report: None,
            export_options: None,
            export_report: None,
        }
    }

    /// Adiciona unidades ao job
    pub fn add_units(&mut self, units: Vec<Unit>) {
        for mut unit in units {
            unit.job_id = self.id;
            self.sections.push(unit.section);
            self.units.insert(unit.id, unit);
        }
        self.sections.sort();
        self.sections.dedup();
        self.update_progress();
    }

    /// Obtém unidade por ID
    pub fn unit(&self, id: &UnitId) -> Option<&Unit> {
        self.units.get(id)
    }

    /// Obtém unidade mutável por ID
    pub fn unit_mut(&mut self, id: &UnitId) -> Option<&mut Unit> {
        self.units.get_mut(id)
    }

    /// Inicia o job
    pub fn start(&mut self) {
        self.state = JobState::Running;
        self.started_at = Some(SystemTime::now());
        self.updated_at = SystemTime::now();
    }

    /// Pausa o job
    pub fn pause(&mut self) {
        if self.state == JobState::Running {
            self.state = JobState::Paused;
            self.updated_at = SystemTime::now();
        }
    }

    /// Retoma o job
    pub fn resume(&mut self) {
        if self.state == JobState::Paused {
            self.state = JobState::Running;
            self.updated_at = SystemTime::now();
        }
    }

    /// Marca como completado
    pub fn complete(&mut self, export_report: super::ExportReport) {
        self.state = JobState::Completed;
        self.completed_at = Some(SystemTime::now());
        self.export_report = Some(export_report);
        let completed = self.units.values().filter(|u| u.is_translated()).count();
        let total = self.units.len();
        let percent = if total > 0 {
            (completed as f64 / total as f64 * 100.0) as u8
        } else {
            0
        };
        self.progress = JobProgress {
            total,
            completed,
            failed: self.units.values().filter(|u| u.state == UnitState::Failed).count(),
            percent,
            ..Default::default()
        };
        self.updated_at = SystemTime::now();
    }

    /// Marca como falhado
    pub fn fail(&mut self, error: String) {
        self.state = JobState::Failed(error);
        self.updated_at = SystemTime::now();
    }

    /// Cancela o job
    pub fn cancel(&mut self) {
        self.state = JobState::Cancelled;
        self.updated_at = SystemTime::now();
    }

    /// Atualiza progresso
    pub fn update_progress(&mut self) {
        let total = self.units.len();
        let completed = self.units.values().filter(|u| u.is_translated()).count();
        let failed = self.units.values().filter(|u| u.state == UnitState::Failed).count();
        let flagged = self.units.values().filter(|u| u.state == UnitState::Flagged).count();
        let pending = total.saturating_sub(completed + failed);

        let percent = if total > 0 {
            (completed as f64 / total as f64 * 100.0) as u8
        } else {
            0
        };

        let elapsed = self.started_at.map(|s| SystemTime::now().duration_since(s).unwrap_or_default());

                eprintln!("DEBUG complete: units={}, completed={}", self.units.len(), self.units.values().filter(|u| u.is_translated()).count());
        self.progress = JobProgress {
            total,
            completed,
            failed,
            flagged,
            pending,
            percent,
            elapsed: elapsed.unwrap_or_default(),
            eta: estimate_eta(elapsed.unwrap_or_default(), completed, total),
            current_section: self.current_section(),
            units_per_second: calculate_ups(elapsed.unwrap_or_default(), completed),
        };
        self.updated_at = SystemTime::now();
    }

    fn current_section(&self) -> Option<SectionId> {
        self.units.values()
            .filter(|u| u.state == UnitState::Pending)
            .min_by_key(|u| u.section)
            .map(|u| u.section)
    }

    /// Verifica se pode retomar (tem unidades pendentes)
    pub fn can_resume(&self) -> bool {
        matches!(self.state, JobState::Paused | JobState::Created | JobState::Running)
            && self.units.values().any(|u| u.state == UnitState::Pending)
    }

    /// Retorna unidades pendentes (para processamento em lote)
    pub fn pending_units(&self, limit: usize) -> Vec<&Unit> {
        self.units.values()
            .filter(|u| u.state == UnitState::Pending)
            .take(limit)
            .collect()
    }

    /// Retorna unidades que falharam (para retry)
    pub fn failed_units(&self) -> Vec<&Unit> {
        self.units.values()
            .filter(|u| u.state == UnitState::Failed)
            .collect()
    }

    /// Duração total se completado
    pub fn duration(&self) -> Option<Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => end.duration_since(start).ok(),
            _ => None,
        }
    }
}

/// Configuração do job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobConfig {
    /// Par de idiomas
    pub lang_pair: LangPair,
    /// ID do modelo a usar
    pub model_id: String,
    /// Provedor (openai-compat, ollama, embedded)
    pub provider: String,
    /// Glossário opcional
    pub glossary: Option<super::Glossary>,
    /// Guia de estilo opcional
    pub style_guide: Option<super::StyleGuide>,
    /// Tamanho do lote (unidades por requisição)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Max requisições paralelas
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Temperatura
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Max tokens de resposta por unidade
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Habilitar QA automático
    #[serde(default = "default_true")]
    pub enable_qa: bool,
    /// Habilitar memória de tradução
    #[serde(default = "default_true")]
    pub enable_tm: bool,
    /// Formato de saída
    pub export_options: Option<ExportOptions>,
}

fn default_batch_size() -> usize { 10 }
fn default_max_parallel() -> usize { 3 }
fn default_temperature() -> f32 { 0.3 }
fn default_max_tokens() -> usize { 2048 }
fn default_true() -> bool { true }

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            lang_pair: LangPair {
                source: super::Language::new("auto"),
                target: super::Language::new("pt-BR"),
            },
            model_id: "auto".into(),
            provider: "openai-compat".into(),
            glossary: None,
            style_guide: None,
            batch_size: default_batch_size(),
            max_parallel: default_max_parallel(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            enable_qa: true,
            enable_tm: true,
            export_options: None,
        }
    }
}

/// Estado do job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum JobState {
    Created,
    Running,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}

/// Progresso do job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JobProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub flagged: usize,
    pub pending: usize,
    pub percent: u8,
    pub elapsed: Duration,
    pub eta: Option<Duration>,
    pub current_section: Option<SectionId>,
    pub units_per_second: f64,
}

fn estimate_eta(elapsed: Duration, completed: usize, total: usize) -> Option<Duration> {
    if completed == 0 || elapsed.as_secs() == 0 {
        return None;
    }
    let rate = completed as f64 / elapsed.as_secs_f64();
    let remaining = total.saturating_sub(completed);
    let eta_secs = remaining as f64 / rate;
    Some(Duration::from_secs_f64(eta_secs))
}

fn calculate_ups(elapsed: Duration, completed: usize) -> f64 {
    if elapsed.as_secs() == 0 {
        0.0
    } else {
        completed as f64 / elapsed.as_secs_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Language, Unit, UnitKind, MarkedText, SourceRef, SectionId};

    fn make_job() -> Job {
        Job::new(JobConfig::default())
    }

    fn make_unit(text: &str) -> Unit {
        let source_ref = SourceRef {
            internal_path: "test.xhtml".into(),
            selector: "//p[1]".into(),
            offset: None,
            length: None,
        };
        Unit::new(source_ref, UnitKind::Paragraph, MarkedText::new(text), SectionId::new(), Uuid::nil())
    }

    #[test]
    fn job_creation() {
        let job = make_job();
        assert_eq!(job.state, JobState::Created);
        assert_eq!(job.units.len(), 0);
    }

    #[test]
    fn job_add_units() {
        let mut job = make_job();
        let units = vec![make_unit("One"), make_unit("Two"), make_unit("Three")];
        job.add_units(units);

        assert_eq!(job.units.len(), 3);
        assert_eq!(job.progress.total, 3);
        assert_eq!(job.progress.pending, 3);
    }

    #[test]
    fn job_start_pause_resume() {
        let mut job = make_job();
        job.start();
        assert_eq!(job.state, JobState::Running);
        assert!(job.started_at.is_some());

        job.pause();
        assert_eq!(job.state, JobState::Paused);

        job.resume();
        assert_eq!(job.state, JobState::Running);
    }

    #[test]
#[test]
fn job_complete() {
    let mut job = make_job();
    let units = vec![make_unit("One"), make_unit("Two")];
    job.add_units(units);

    println!("Before translation - units: {}", job.units.len());
    for (id, unit) in &job.units {
        println!("Unit {}: state={:?}, is_translated={}", id, unit.state, unit.is_translated());
    }

    // Simula tradução
    for unit in job.units.values_mut() {
        unit.apply_translation(MarkedText::new("Traduzido")).unwrap();
    }
    
    println!("After translation:");
    for (id, unit) in &job.units {
        println!("Unit {}: state={:?}, is_translated={}", id, unit.state, unit.is_translated());
    }

    let export_report = super::super::ExportReport {
        output_path: "out.epub".into(),
        format: super::super::OutputFormat::EPUB,
        units_processed: 2,
        units_with_warnings: 0,
        units_failed: 0,
        output_size: 1024,
        export_time_ms: 100,
        warnings: vec![],
    };

    job.complete(export_report);
    assert_eq!(job.state, JobState::Completed);
    println!("Progress: {}%", job.progress.percent);
    assert_eq!(job.progress.percent, 100);
}

    #[test]
    fn job_pending_units() {
        let mut job = make_job();
        let units = vec![make_unit("One"), make_unit("Two"), make_unit("Three")];
        job.add_units(units);

        // Traduz uma
        if let Some(unit) = job.units.values_mut().next() {
            unit.apply_translation(MarkedText::new("Traduzido")).unwrap();
        }

        let pending = job.pending_units(10);
        assert_eq!(pending.len(), 2);
    }
}