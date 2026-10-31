//! Projeto - agrega jobs, configurações globais e metadados

use super::{ExportOptions, FidelityReport, Job, JobConfig, Language, Unit};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

/// Projeto de tradução
///
/// Contém um ou mais jobs (ex: mesmo livro para vários idiomas),
/// configurações globais e metadados do arquivo fonte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub source_file: PathBuf,
    pub source_format: String,
    pub detected_language: Option<Language>,
    pub fidelity_report: Option<FidelityReport>,
    pub jobs: HashMap<Uuid, Job>,
    pub global_glossary: Option<super::Glossary>,
    pub global_style_guide: Option<super::StyleGuide>,
    pub export_options: ExportOptions,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub metadata: ProjectMetadata,
}

impl Project {
    /// Cria novo projeto a partir de arquivo
    pub fn new(name: impl Into<String>, source_file: PathBuf, format: impl Into<String>) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            source_file,
            source_format: format.into(),
            detected_language: None,
            fidelity_report: None,
            jobs: HashMap::new(),
            global_glossary: None,
            global_style_guide: None,
            export_options: ExportOptions::default(),
            created_at: now,
            updated_at: now,
            metadata: ProjectMetadata::default(),
        }
    }

    /// Adiciona um job ao projeto
    pub fn add_job(&mut self, config: JobConfig) -> Uuid {
        let job = Job::new(config);
        let id = job.id;
        self.jobs.insert(id, job);
        self.updated_at = SystemTime::now();
        id
    }

    /// Obtém job por ID
    pub fn job(&self, id: &Uuid) -> Option<&Job> {
        self.jobs.get(id)
    }

    /// Obtém job mutável por ID
    pub fn job_mut(&mut self, id: &Uuid) -> Option<&mut Job> {
        self.jobs.get_mut(id)
    }

    /// Lista todos os jobs
    pub fn jobs(&self) -> Vec<&Job> {
        self.jobs.values().collect()
    }

    /// Job ativo (mais recente não completado)
    pub fn active_job(&self) -> Option<&Job> {
        self.jobs.values()
            .filter(|j| !matches!(j.state, super::JobState::Completed | super::JobState::Cancelled))
            .max_by_key(|j| j.updated_at)
    }

    /// Atualiza metadados do arquivo fonte
    pub fn set_source_info(&mut self, language: Language, fidelity: FidelityReport) {
        self.detected_language = Some(language);
        self.fidelity_report = Some(fidelity);
        self.updated_at = SystemTime::now();
    }

    /// Total de unidades em todos os jobs
    pub fn total_units(&self) -> usize {
        self.jobs.values().map(|j| j.units.len()).sum()
    }

    /// Unidades completadas em todos os jobs
    pub fn completed_units(&self) -> usize {
        self.jobs.values()
            .flat_map(|j| j.units.values())
            .filter(|u| u.is_translated())
            .count()
    }

    /// Progresso geral (0-100)
    pub fn overall_progress(&self) -> u8 {
        let total = self.total_units();
        if total == 0 { return 0; }
        ((self.completed_units() as f64 / total as f64) * 100.0) as u8
    }
}

/// Metadados do projeto
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub publisher: Option<String>,
    pub publication_date: Option<String>,
    pub isbn: Option<String>,
    pub description: Option<String>,
    pub cover_image: Option<PathBuf>,
    pub tags: Vec<String>,
    pub custom: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn project_creation() {
        let project = Project::new("Meu Livro", PathBuf::from("book.epub"), "EPUB");
        assert_eq!(project.name, "Meu Livro");
        assert_eq!(project.source_format, "EPUB");
        assert!(project.jobs.is_empty());
    }

    #[test]
    fn project_add_job() {
        let mut project = Project::new("Test", PathBuf::from("test.txt"), "TXT");
        let job_id = project.add_job(JobConfig::default());

        assert!(project.job(&job_id).is_some());
        assert_eq!(project.jobs.len(), 1);
    }

    #[test]
    fn project_progress() {
        let mut project = Project::new("Test", PathBuf::from("test.txt"), "TXT");
        let job_id = project.add_job(JobConfig::default());

        // Adiciona unidades ao job
        if let Some(job) = project.job_mut(&job_id) {
            use crate::{Unit, UnitKind, MarkedText, SourceRef, SectionId};
            let units = vec![
                Unit::new(SourceRef { internal_path: "test.txt".into(), selector: "1".into(), offset: None, length: None }, UnitKind::Paragraph, MarkedText::new("One"), SectionId::new()),
                Unit::new(SourceRef { internal_path: "test.txt".into(), selector: "2".into(), offset: None, length: None }, UnitKind::Paragraph, MarkedText::new("Two"), SectionId::new()),
            ];
            job.add_units(units);
        }

        assert_eq!(project.total_units(), 2);
        assert_eq!(project.completed_units(), 0);
        assert_eq!(project.overall_progress(), 0);
    }
}