//! Adaptador de armazenamento SQLite

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use pgl_app::{ProjectRepository, StorageError, Project, Job, Unit, UnitId, UnitState, ProjectSummary, JobSummary, Glossary, ExportOptions};
use pgl_domain::{Language, SectionId, LangPair};
use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
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
        // Run migrations from embedded SQL
        // Use sqlx's built-in migration support for proper handling
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        Ok(())
    }

    fn system_time_to_i64(time: SystemTime) -> i64 {
        time.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
    }

    fn i64_to_system_time(secs: i64) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    }

    async fn save_project(&self, project: &Project) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(|e| StorageError::Sqlx(e.into()))?;

        // Prepare all values upfront to avoid temporary value issues
        let fidelity_json = serde_json::to_string(&project.fidelity_report).unwrap();
        let export_json = serde_json::to_string(&project.export_options).unwrap();
        let metadata_json = serde_json::to_string(&project.metadata).unwrap();
        let detected_lang = project.detected_language.as_ref().map(|l| l.code());
        let created_at = Self::system_time_to_i64(project.created_at);
        let updated_at = Self::system_time_to_i64(project.updated_at);
        let id_str = project.id.to_string();
        let source_file_str = project.source_file.to_string_lossy().to_string();
        let source_format = &project.source_format;

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
            id_str,
            project.name,
            source_file_str,
            source_format,
            detected_lang,
            fidelity_json,
            export_json,
            created_at,
            updated_at,
            metadata_json,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        tx.commit().await.map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_project(&self, id: &Uuid) -> Result<Option<Project>, StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, name, source_file, source_format, detected_language, fidelity_report, export_options, created_at, updated_at, metadata, global_glossary, global_style_guide FROM projects WHERE id = ?",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            // Load jobs for this project
            let project_id_str = id.to_string();
            let job_rows = sqlx::query!(
                "SELECT id, project_id, config, state, progress, fidelity_report, export_options, export_report, created_at, updated_at, started_at, completed_at FROM jobs WHERE project_id = ?",
                project_id_str
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Sqlx(e.into()))?;

            let mut jobs = HashMap::new();
            for job_row in job_rows {
                let job = Job {
                    id: Uuid::parse_str(job_row.id.as_deref().unwrap_or("")).unwrap().into(),
                    project_id: Uuid::parse_str(job_row.project_id.as_ref()).unwrap().into(),
                    config: serde_json::from_str(&job_row.config).unwrap(),
                    state: serde_json::from_str(&job_row.state).unwrap(),
                    progress: serde_json::from_str(&job_row.progress).unwrap(),
                    fidelity_report: job_row.fidelity_report.as_deref().and_then(|s: &str| {
                        if s == "null" {
                            None
                        } else {
                            serde_json::from_str(s).ok()
                        }
                    }),
                    export_options: job_row.export_options.as_deref().and_then(|s: &str| {
                        if s == "null" {
                            None
                        } else {
                            serde_json::from_str(s).ok()
                        }
                    }),
                    export_report: job_row.export_report.as_deref().and_then(|s: &str| {
                        if s == "null" {
                            None
                        } else {
                            serde_json::from_str(s).ok()
                        }
                    }),
                    created_at: Self::i64_to_system_time(job_row.created_at),
                    updated_at: Self::i64_to_system_time(job_row.updated_at),
                    started_at: job_row.started_at.map(Self::i64_to_system_time),
                    completed_at: job_row.completed_at.map(Self::i64_to_system_time),
                    units: HashMap::new(),
                    sections: Vec::new(),
                };
                jobs.insert(job.id, job);
            }

            let project = Project {
                id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
                name: row.name,
                source_file: PathBuf::from(&row.source_file),
                source_format: row.source_format,
                detected_language: row.detected_language.map(Language::new),
                fidelity_report: row.fidelity_report.as_deref().and_then(|s: &str| {
                    if s == "null" {
                        None
                    } else {
                        serde_json::from_str(s).ok()
                    }
                }),
                export_options: serde_json::from_str(&row.export_options).unwrap(),
                created_at: Self::i64_to_system_time(row.created_at),
                updated_at: Self::i64_to_system_time(row.updated_at),
                metadata: serde_json::from_str(&row.metadata).unwrap(),
                global_glossary: row.global_glossary.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                global_style_guide: row.global_style_guide.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                jobs,
            };
            Ok(Some(project))
        } else {
            Ok(None)
        }
    }

    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        let rows = sqlx::query!(
            "SELECT id, name, source_format, detected_language, created_at, updated_at FROM projects ORDER BY updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| ProjectSummary {
            id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
            name: row.name,
            source_file: "".to_string(),
            source_format: row.source_format,
            created_at: chrono::DateTime::from_timestamp(row.created_at, 0).unwrap().to_rfc3339(),
            updated_at: chrono::DateTime::from_timestamp(row.updated_at, 0).unwrap().to_rfc3339(),
            overall_progress: 0,
        }).collect())
    }

    async fn delete_project(&self, id: &Uuid) -> Result<(), StorageError> {
        let id_str = id.to_string();
        sqlx::query!("DELETE FROM projects WHERE id = ?", id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn save_job(&self, job: &Job, project_id: &Uuid) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(|e| StorageError::Sqlx(e.into()))?;

        let config_json = serde_json::to_string(&job.config).unwrap();
        let state_json = serde_json::to_string(&job.state).unwrap();
        let progress_json = serde_json::to_string(&job.progress).unwrap();
        let fidelity_json = serde_json::to_string(&job.fidelity_report).unwrap();
        let export_opts_json = serde_json::to_string(&job.export_options).unwrap();
        let export_report_json = serde_json::to_string(&job.export_report).unwrap();
        let created_at = Self::system_time_to_i64(job.created_at);
        let updated_at = Self::system_time_to_i64(job.updated_at);
        let started_at = job.started_at.map(Self::system_time_to_i64);
        let completed_at = job.completed_at.map(Self::system_time_to_i64);
        let id_str = job.id.to_string();
        let project_id_str = project_id.to_string();

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
            id_str,
            project_id_str,
            config_json,
            state_json,
            progress_json,
            fidelity_json,
            export_opts_json,
            export_report_json,
            created_at,
            updated_at,
            started_at,
            completed_at,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        tx.commit().await.map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_job(&self, id: &Uuid) -> Result<Option<Job>, StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, project_id, config, state, progress, fidelity_report, export_options, export_report, created_at, updated_at, started_at, completed_at FROM jobs WHERE id = ?",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let job = Job {
                id: Uuid::parse_str(&id_str).unwrap().into(),
                project_id: Uuid::parse_str(&row.project_id).unwrap().into(),
                config: serde_json::from_str(&row.config).unwrap(),
                state: serde_json::from_str(&row.state).unwrap(),
                progress: serde_json::from_str(&row.progress).unwrap(),
                fidelity_report: row.fidelity_report.as_deref().and_then(|s: &str| {
                    if s == "null" {
                        None
                    } else {
                        serde_json::from_str(s).ok()
                    }
                }),
                export_options: row.export_options.as_deref().and_then(|s: &str| {
                    if s == "null" {
                        None
                    } else {
                        serde_json::from_str(s).ok()
                    }
                }),
                export_report: row.export_report.as_deref().and_then(|s: &str| {
                    if s == "null" {
                        None
                    } else {
                        serde_json::from_str(s).ok()
                    }
                }),
                created_at: Self::i64_to_system_time(row.created_at),
                updated_at: Self::i64_to_system_time(row.updated_at),
                started_at: row.started_at.map(Self::i64_to_system_time),
                completed_at: row.completed_at.map(Self::i64_to_system_time),
                units: HashMap::new(),
                sections: Vec::new(),
            };
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn load_jobs(&self, project_id: &Uuid) -> Result<Vec<JobSummary>, StorageError> {
        let project_id_str = project_id.to_string();
        let rows = sqlx::query!(
            "SELECT id, config, state, progress, created_at, updated_at FROM jobs WHERE project_id = ? ORDER BY created_at DESC",
            project_id_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| {
            let config: serde_json::Value = serde_json::from_str(&row.config).unwrap();
            JobSummary {
                id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap(),
                project_id: *project_id,
                target_language: Language::new(config.get("lang_pair").and_then(|c| c.get("target")).and_then(|t| t.get("0")).and_then(|v| v.as_str()).unwrap_or("pt-BR")),
                state: serde_json::from_str(&row.state).unwrap(),
                progress: serde_json::from_str(&row.progress).unwrap(),
                created_at: chrono::DateTime::from_timestamp(row.created_at, 0).unwrap().to_rfc3339(),
                updated_at: chrono::DateTime::from_timestamp(row.updated_at, 0).unwrap().to_rfc3339(),
            }
        }).collect())
    }

    async fn save_unit(&self, unit: &Unit, job_id: &Uuid) -> Result<(), StorageError> {
        let id_str = unit.id.to_string();
        let source_ref_json = serde_json::to_string(&unit.source_ref).unwrap();
        let kind_json = serde_json::to_string(&unit.kind).unwrap();
        let text_json = serde_json::to_string(&unit.text).unwrap();
        let section_str = unit.section.to_string();
        let state_json = serde_json::to_string(&unit.state).unwrap();
        let translation_json = unit.translation.as_ref().map(|t| serde_json::to_string(t).unwrap());
        let qa_warnings_json = serde_json::to_string(&unit.qa_warnings).unwrap();
        let retry_count = unit.retry_count as i64;
        let metadata_json = serde_json::to_string(&unit.metadata).unwrap();
        let job_id_str = job_id.to_string();

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
            id_str,
            job_id_str,
            source_ref_json,
            kind_json,
            text_json,
            section_str,
            state_json,
            translation_json,
            qa_warnings_json,
            retry_count,
            metadata_json,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_unit(&self, id: &UnitId) -> Result<Option<Unit>, StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, job_id, source_ref, kind, text, section, state, translation, qa_warnings, retry_count, metadata FROM units WHERE id = ?",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let unit = Unit {
                id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
                job_id: {
                let job_id_str: &str = row.job_id.as_str();
                Uuid::parse_str(job_id_str).unwrap().into()
            },
                source_ref: serde_json::from_str(&row.source_ref).unwrap(),
                kind: serde_json::from_str(&row.kind).unwrap(),
                text: serde_json::from_str(&row.text).unwrap(),
                section: SectionId(Uuid::parse_str(&row.section).unwrap()),
                state: serde_json::from_str(&row.state).unwrap(),
                translation: row.translation.as_deref().map(|t| serde_json::from_str(t).unwrap()),
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
        let job_id_str = job_id.to_string();
        let rows = sqlx::query!(
            "SELECT id, source_ref, kind, text, section, state, translation, qa_warnings, retry_count, metadata FROM units WHERE job_id = ? AND state IN (SELECT value FROM json_each(?))",
            job_id_str,
            states_json
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| Unit {
            id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
            job_id: *job_id,
            source_ref: serde_json::from_str(&row.source_ref).unwrap(),
            kind: serde_json::from_str(&row.kind).unwrap(),
            text: serde_json::from_str(&row.text).unwrap(),
            section: SectionId(Uuid::parse_str(&row.section).unwrap()),
            state: serde_json::from_str(&row.state).unwrap(),
            translation: row.translation.as_deref().map(|t| serde_json::from_str(t).unwrap()),
            qa_warnings: serde_json::from_str(&row.qa_warnings).unwrap(),
            retry_count: row.retry_count as u32,
            metadata: serde_json::from_str(&row.metadata).unwrap(),
        }).collect())
    }

    async fn update_unit_state(&self, id: &UnitId, state: UnitState) -> Result<(), StorageError> {
        let state_json = serde_json::to_string(&state).unwrap();
        let id_str = id.to_string();
        sqlx::query!(
            "UPDATE units SET state = ? WHERE id = ?",
            state_json,
            id_str
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn update_unit_translation(&self, id: &UnitId, translation: &pgl_domain::MarkedText, warnings: &[pgl_domain::QaWarning]) -> Result<(), StorageError> {
        let translation_json = serde_json::to_string(translation).unwrap();
        let warnings_json = serde_json::to_string(warnings).unwrap();
        let id_str = id.to_string();
        sqlx::query!(
            "UPDATE units SET translation = ?, qa_warnings = ? WHERE id = ?",
            translation_json,
            warnings_json,
            id_str
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn save_glossary(&self, glossary: &Glossary) -> Result<(), StorageError> {
        let entries_json = serde_json::to_string(&glossary.entries).unwrap();
        let metadata_json = serde_json::to_string(&glossary.metadata).unwrap();
        let created_at = glossary.metadata.created_at.clone().unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let updated_at = glossary.metadata.updated_at.clone().unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let id_str = glossary.id.to_string();
        let project_id_str = glossary.project_id.as_ref().map(|u| u.to_string());
        let job_id_str = glossary.job_id.as_ref().map(|u| u.to_string());
        let created_at_str = created_at.clone();
        let updated_at_str = updated_at.clone();
        let lang_source = glossary.lang_pair.source.code();
        let lang_target = glossary.lang_pair.target.code();

        sqlx::query!(
            r#"
            INSERT INTO glossaries (id, project_id, job_id, name, lang_pair_source, lang_pair_target, entries, metadata, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            id_str,
            project_id_str,
            job_id_str,
            glossary.name,
            lang_source,
            lang_target,
            entries_json,
            metadata_json,
            created_at_str,
            updated_at_str,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_glossary(&self, id: &Uuid) -> Result<Option<Glossary>, StorageError> {
        let id_str = id.to_string();
        let row = sqlx::query!(
            "SELECT id, project_id, job_id, name, lang_pair_source, lang_pair_target, entries, metadata, created_at, updated_at FROM glossaries WHERE id = ?",
            id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let glossary = Glossary {
                id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
                project_id: row.project_id.as_ref().map(|s| Uuid::parse_str(s).unwrap().into()),
                job_id: row.job_id.as_ref().map(|s| Uuid::parse_str(s).unwrap().into()),
                name: row.name,
                lang_pair: LangPair { source: Language::new(&row.lang_pair_source), target: Language::new(&row.lang_pair_target) },
                entries: serde_json::from_str(&row.entries).unwrap(),
                metadata: serde_json::from_str(&row.metadata).unwrap(),
                created_at: Self::i64_to_system_time(row.created_at),
                updated_at: Self::i64_to_system_time(row.updated_at),
            };
            Ok(Some(glossary))
        } else {
            Ok(None)
        }
    }

    async fn list_glossaries(&self, project_id: &Uuid) -> Result<Vec<Glossary>, StorageError> {
        let project_id_str = project_id.to_string();
        let rows = sqlx::query!(
            "SELECT id, project_id, job_id, name, lang_pair_source, lang_pair_target, entries, metadata, created_at, updated_at FROM glossaries WHERE project_id = ?",
            project_id_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| Glossary {
            id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
            project_id: row.project_id.as_ref().map(|s| Uuid::parse_str(s).unwrap().into()),
            job_id: row.job_id.as_ref().map(|s| Uuid::parse_str(s).unwrap().into()),
            name: row.name,
            lang_pair: LangPair { source: Language::new(&row.lang_pair_source), target: Language::new(&row.lang_pair_target) },
            entries: serde_json::from_str(&row.entries).unwrap(),
            metadata: serde_json::from_str(&row.metadata).unwrap(),
            created_at: Self::i64_to_system_time(row.created_at),
            updated_at: Self::i64_to_system_time(row.updated_at),
        }).collect())
    }

    async fn save_asset(&self, _asset: &pgl_app::AssetRef) -> Result<(), StorageError> {
        Ok(())
    }

    async fn get_asset(&self, _id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(None)
    }
}

#[async_trait]
impl ProjectRepository for SqliteRepository {
    async fn save_project(&self, project: &Project) -> Result<(), StorageError> {
        self.save_project(project).await
    }

    async fn load_project(&self, id: &Uuid) -> Result<Option<Project>, StorageError> {
        self.load_project(id).await
    }

    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        self.list_projects().await
    }

    async fn delete_project(&self, id: &Uuid) -> Result<(), StorageError> {
        self.delete_project(id).await
    }

    async fn save_job(&self, job: &Job, project_id: &Uuid) -> Result<(), StorageError> {
        self.save_job(job, project_id).await
    }

    async fn load_job(&self, id: &Uuid) -> Result<Option<Job>, StorageError> {
        self.load_job(id).await
    }

    async fn list_jobs(&self, project_id: &Uuid) -> Result<Vec<JobSummary>, StorageError> {
        self.load_jobs(project_id).await
    }

    async fn save_unit(&self, unit: &Unit, job_id: &Uuid) -> Result<(), StorageError> {
        self.save_unit(unit, job_id).await
    }

    async fn load_unit(&self, id: &UnitId) -> Result<Option<Unit>, StorageError> {
        self.load_unit(id).await
    }

    async fn load_units(&self, job_id: &Uuid, states: &[UnitState]) -> Result<Vec<Unit>, StorageError> {
        self.load_units(job_id, states).await
    }

    async fn update_unit_state(&self, id: &UnitId, state: UnitState) -> Result<(), StorageError> {
        self.update_unit_state(id, state).await
    }

    async fn update_unit_translation(&self, id: &UnitId, translation: &pgl_domain::MarkedText, warnings: &[pgl_domain::QaWarning]) -> Result<(), StorageError> {
        self.update_unit_translation(id, translation, warnings).await
    }

    async fn find_project_for_job(&self, job_id: &Uuid) -> Result<Option<Project>, StorageError> {
        let row = sqlx::query(
            "SELECT p.id, p.name, p.source_file, p.source_format, p.detected_language, 
                    p.fidelity_report, p.global_glossary, p.global_style_guide, 
                    p.export_options, p.created_at, p.updated_at, p.metadata
             FROM projects p
             JOIN jobs j ON j.project_id = p.id
             WHERE j.id = ?"
        )
        .bind(job_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let fidelity_report: Option<String> = row.get("fidelity_report");
            let global_glossary: Option<String> = row.get("global_glossary");
            let global_style_guide: Option<String> = row.get("global_style_guide");
            let export_options: Option<String> = row.get("export_options");
            let metadata: Option<String> = row.get("metadata");

            let fidelity_report = fidelity_report.as_deref().and_then(|s| serde_json::from_str(s).ok());
            let global_glossary = global_glossary.as_deref().and_then(|s| serde_json::from_str(s).ok());
            let global_style_guide = global_style_guide.as_deref().and_then(|s| serde_json::from_str(s).ok());
            let export_options: ExportOptions = export_options.as_deref().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
            let metadata = metadata.as_deref().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();

            Ok(Some(Project {
                id: Uuid::from_slice(&row.get::<Vec<u8>, _>("id")).map_err(|e| StorageError::Serialization(e.to_string()))?,
                name: row.get("name"),
                source_file: PathBuf::from(row.get::<String, _>("source_file")),
                source_format: row.get("source_format"),
                detected_language: row.get::<Option<String>, _>("detected_language").map(Language),
                fidelity_report,
                jobs: HashMap::new(),
                global_glossary,
                global_style_guide,
                export_options,
                created_at: Self::i64_to_system_time(row.get::<i64, _>("created_at")),
                updated_at: Self::i64_to_system_time(row.get::<i64, _>("updated_at")),
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

}