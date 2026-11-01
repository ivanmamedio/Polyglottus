//! Adaptador de armazenamento SQLite

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

pub mod schema;
pub mod migrations;
pub mod project_repo;
pub mod job_repo;
pub mod unit_repo;
pub mod glossary_repo;
pub mod tm_repo;

use pgl_app::{ProjectRepository, StorageError, Project, Job, Unit, UnitId, UnitState, ProjectSummary, JobSummary, Glossary};
use pgl_domain::{Language, SectionId};
use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use std::sync::Arc;
use std::path::Path;
use uuid::Uuid;
use chrono::DateTime;
use tracing::{info, debug};

/// Repositório SQLite
pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    /// Cria novo repositório
    pub async fn new(db_path: &Path) -> Result<Self, StorageError> {
        // Garante diretório
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&db_url).await
            .map_err(|e| StorageError::Sqlx(e.into()))?;

        // Executa migrações
        Self::run_migrations(&pool).await?;

        info!("Database initialized: {}", db_path.display());

        Ok(Self { pool })
    }

    async fn run_migrations(pool: &SqlitePool) -> Result<(), StorageError> {
        // Migrações são aplicadas via sqlx::migrate
        sqlx::migrate!("./migrations").run(pool).await
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        Ok(())
    }

    /// Retorna pool para uso direto se necessário
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl ProjectRepository for SqliteRepository {
    async fn save_project(&self, project: &Project) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(|e| StorageError::Sqlx(e.into()))?;

        // Upsert project
        sqlx::query!(
            r#"
            INSERT INTO projects (id, name, source_file, source_format, detected_language, fidelity_report, export_options, created_at, updated_at, metadata)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                source_file = excluded.source_file,
                source_format = excluded.source_format,
                detected_language = excluded.detected_language,
                fidelity_report = excluded.fidelity_report,
                export_options = excluded.export_options,
                updated_at = excluded.updated_at,
                metadata = excluded.metadata
            "#,
            project.id.to_string(),
            project.name,
            project.source_file.to_string_lossy().to_string(),
            project.source_format,
            project.detected_language.as_ref().map(|l| l.code()),
            serde_json::to_string(&project.fidelity_report).unwrap(),
            serde_json::to_string(&project.export_options).unwrap(),
            project.created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            project.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            serde_json::to_string(&project.metadata).unwrap(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        tx.commit().await.map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_project(&self, id: &Uuid) -> Result<Option<Project>, StorageError> {
        let row = sqlx::query!(
            "SELECT id, name, source_file, source_format, detected_language, fidelity_report, export_options, created_at, updated_at, metadata, global_glossary, global_style_guide FROM projects WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let project = Project {
                id: Uuid::parse_str(&row.id).unwrap(),
                name: row.name,
                source_file: std::path::PathBuf::from(row.source_file),
                source_format: row.source_format,
                detected_language: row.detected_language.map(Language::new),
                fidelity_report: serde_json::from_str(&row.fidelity_report).ok(),
                export_options: serde_json::from_str(&row.export_options).unwrap_or_default(),
                created_at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(row.created_at as u64),
                updated_at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(row.updated_at as u64),
                metadata: serde_json::from_str(&row.metadata).unwrap_or_default(),
                jobs: HashMap::new(), // Carregado separadamente
                global_glossary: row.global_glossary.map(|g| serde_json::from_str(g).unwrap()),
                global_style_guide: row.global_style_guide.map(|g| serde_json::from_str(g).unwrap()),
            };
            Ok(Some(project))
        } else {
            Ok(None)
        }
    }

    async fn delete_project(&self, id: &Uuid) -> Result<(), StorageError> {
        sqlx::query!("DELETE FROM projects WHERE id = ?", id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        let rows = sqlx::query!(
            "SELECT id, name, source_file, source_format, created_at, updated_at FROM projects ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| ProjectSummary {
            id: Uuid::parse_str(&row.id).unwrap(),
            name: row.name,
            source_file: row.source_file,
            source_format: row.source_format,
            created_at: DateTime::from_timestamp(row.created_at, 0).unwrap().to_rfc3339(),
            updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap().to_rfc3339(),
            overall_progress: 0, // Calculado separadamente
        }).collect())
    }

    async fn save_job(&self, job: &Job) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(|e| StorageError::Sqlx(e.into()))?;

        sqlx::query!(
            r#"
            INSERT INTO jobs (id, project_id, config, state, progress, fidelity_report, export_options, export_report, created_at, updated_at, started_at, completed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                config = excluded.config,
                state = excluded.state,
                progress = excluded.progress,
                fidelity_report = excluded.fidelity_report,
                export_options = excluded.export_options,
                export_report = excluded.export_report,
                updated_at = excluded.updated_at,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at
            "#,
            job.id.to_string(),
            // project_id seria necessário no Job
            "".to_string(), // placeholder
            serde_json::to_string(&job.config).unwrap(),
            serde_json::to_string(&job.state).unwrap(),
            serde_json::to_string(&job.progress).unwrap(),
            serde_json::to_string(&job.fidelity_report).unwrap(),
            serde_json::to_string(&job.export_options).unwrap(),
            serde_json::to_string(&job.export_report).unwrap(),
            job.created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            job.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64,
            job.started_at.map(|s| s.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64),
            job.completed_at.map(|s| s.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        tx.commit().await.map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_job(&self, id: &Uuid) -> Result<Option<Job>, StorageError> {
        let row = sqlx::query!(
            "SELECT id, config, state, progress, fidelity_report, export_options, export_report, created_at, updated_at, started_at, completed_at FROM jobs WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let job = Job {
                id: Uuid::parse_str(&row.id).unwrap(),
                config: serde_json::from_str(&row.config).unwrap(),
                state: serde_json::from_str(&row.state).unwrap(),
                progress: serde_json::from_str(&row.progress).unwrap(),
                fidelity_report: serde_json::from_str(&row.fidelity_report).ok(),
                export_options: serde_json::from_str(&row.export_options).ok(),
                export_report: serde_json::from_str(&row.export_report).ok(),
                created_at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(row.created_at as u64),
                updated_at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(row.updated_at as u64),
                started_at: row.started_at.map(|s| std::time::UNIX_EPOCH + std::time::Duration::from_secs(s as u64)),
                completed_at: row.completed_at.map(|s| std::time::UNIX_EPOCH + std::time::Duration::from_secs(s as u64)),
                units: HashMap::new(),
                sections: Vec::new(),
            };
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn list_jobs(&self, project_id: &Uuid) -> Result<Vec<JobSummary>, StorageError> {
        let rows = sqlx::query!(
            "SELECT id, config, state, progress, created_at, updated_at FROM jobs WHERE project_id = ? ORDER BY created_at DESC",
            project_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| {
            let config: serde_json::Value = serde_json::from_str(&row.config).unwrap();
            JobSummary {
                id: Uuid::parse_str(&row.id).unwrap(),
                project_id: *project_id,
                target_language: Language::new(config.get("lang_pair").and_then(|c| c.get("target")).and_then(|t| t.get("0")).and_then(|v| v.as_str()).unwrap_or("pt-BR")),
                state: serde_json::from_str(&row.state).unwrap(),
                progress: serde_json::from_str(&row.progress).unwrap(),
                created_at: DateTime::from_timestamp(row.created_at, 0).unwrap().to_rfc3339(),
                updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap().to_rfc3339(),
            }
        }).collect())
    }

    async fn save_unit(&self, unit: &Unit) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            INSERT INTO units (id, job_id, source_ref, kind, text, section, state, translation, qa_warnings, retry_count, metadata)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                source_ref = excluded.source_ref,
                kind = excluded.kind,
                text = excluded.text,
                section = excluded.section,
                state = excluded.state,
                translation = excluded.translation,
                qa_warnings = excluded.qa_warnings,
                retry_count = excluded.retry_count,
                metadata = excluded.metadata
            "#,
            unit.id.to_string(),
            // job_id seria necessário
            "".to_string(),
            serde_json::to_string(&unit.source_ref).unwrap(),
            serde_json::to_string(&unit.kind).unwrap(),
            serde_json::to_string(&unit.text).unwrap(),
            unit.section.to_string(),
            serde_json::to_string(&unit.state).unwrap(),
            unit.translation.as_ref().map(|t| serde_json::to_string(t).unwrap()),
            serde_json::to_string(&unit.qa_warnings).unwrap(),
            unit.retry_count as i64,
            serde_json::to_string(&unit.metadata).unwrap(),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_unit(&self, id: &UnitId) -> Result<Option<Unit>, StorageError> {
        let row = sqlx::query!(
            "SELECT id, source_ref, kind, text, section, state, translation, qa_warnings, retry_count, metadata FROM units WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let unit = Unit {
                id: Uuid::parse_str(&row.id).unwrap().into(),
                source_ref: serde_json::from_str(&row.source_ref).unwrap(),
                kind: serde_json::from_str(&row.kind).unwrap(),
                text: serde_json::from_str(&row.text).unwrap(),
                section: SectionId(Uuid::parse_str(&row.section).unwrap()),
                state: serde_json::from_str(&row.state).unwrap(),
                translation: row.translation.map(|t| serde_json::from_str(t).unwrap()),
                qa_warnings: serde_json::from_str(&row.qa_warnings).unwrap(),
                retry_count: row.retry_count as u32,
                metadata: serde_json::from_str(&row.metadata).unwrap(),
            };
            Ok(Some(unit))
        } else {
            Ok(None)
        }
    }

    async fn load_units(&self, job_id: &Uuid, states: &[UnitState]) -> Result<Vec<Unit>, StorageError> {
        let states_json = serde_json::to_string(states).unwrap();
        let rows = sqlx::query!(
            "SELECT id, source_ref, kind, text, section, state, translation, qa_warnings, retry_count, metadata FROM units WHERE job_id = ? AND state IN (SELECT value FROM json_each(?))",
            job_id.to_string(),
            states_json
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| Unit {
            id: Uuid::parse_str(&row.id).unwrap().into(),
            source_ref: serde_json::from_str(&row.source_ref).unwrap(),
            kind: serde_json::from_str(&row.kind).unwrap(),
            text: serde_json::from_str(&row.text).unwrap(),
            section: SectionId(Uuid::parse_str(&row.section).unwrap()),
            state: serde_json::from_str(&row.state).unwrap(),
            translation: row.translation.map(|t| serde_json::from_str(t).unwrap()),
            qa_warnings: serde_json::from_str(&row.qa_warnings).unwrap(),
            retry_count: row.retry_count as u32,
            metadata: serde_json::from_str(&row.metadata).unwrap(),
        }).collect())
    }

    async fn update_unit_state(&self, id: &UnitId, state: UnitState) -> Result<(), StorageError> {
        sqlx::query!(
            "UPDATE units SET state = ? WHERE id = ?",
            serde_json::to_string(&state).unwrap(),
            id.to_string()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn update_unit_translation(&self, id: &UnitId, translation: &pgl_domain::MarkedText, warnings: &[pgl_domain::QaWarning]) -> Result<(), StorageError> {
        sqlx::query!(
            "UPDATE units SET translation = ?, qa_warnings = ? WHERE id = ?",
            serde_json::to_string(translation).unwrap(),
            serde_json::to_string(warnings).unwrap(),
            id.to_string()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn repository_creation() {
        let temp = NamedTempFile::new().unwrap();
        let repo = SqliteRepository::new(temp.path()).await;
        assert!(repo.is_ok());
    }
}