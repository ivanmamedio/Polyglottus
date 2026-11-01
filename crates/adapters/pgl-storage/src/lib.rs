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
        // Run migrations from embedded SQL
        let migrations = include_str!("../migrations/001_initial_schema.sql");
        for statement in migrations.split(';') {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                sqlx::query(statement)
                    .execute(pool)
                    .await
                    .map_err(|e| StorageError::Migration(e.to_string()))?;
            }
        }
        Ok(())
    }

    async fn save_project(&self, project: &Project) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(|e| StorageError::Sqlx(e.into()))?;

        // Prepare all values upfront to avoid temporary value issues
        let fidelity_json = serde_json::to_string(&project.fidelity_report).unwrap();
        let export_json = serde_json::to_string(&project.export_options).unwrap();
        let metadata_json = serde_json::to_string(&project.metadata).unwrap();
        let detected_lang = project.detected_language.as_ref().map(|l| l.code());
        let created_at = project.created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let updated_at = project.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
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
        let row = sqlx::query!(
            "SELECT id, name, source_file, source_format, detected_language, fidelity_report, export_options, created_at, updated_at, metadata, global_glossary, global_style_guide FROM projects WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let project = Project {
                id: Uuid::parse_str(&row.id).unwrap().into(),
                name: row.name,
                source_file: PathBuf::from(&row.source_file),
                source_format: row.source_format,
                detected_language: row.detected_language.map(Language::new),
                fidelity_report: serde_json::from_str(&row.fidelity_report).unwrap(),
                export_options: serde_json::from_str(&row.export_options).unwrap(),
                created_at: DateTime::from_timestamp(row.created_at, 0).unwrap(),
                updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap(),
                metadata: serde_json::from_str(&row.metadata).unwrap(),
                global_glossary: row.global_glossary.map(|g| serde_json::from_str(&g).unwrap()),
                global_style_guide: row.global_style_guide.map(|g| serde_json::from_str(&g).unwrap()),
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
            id: Uuid::parse_str(&row.id).unwrap().into(),
            name: row.name,
            source_format: row.source_format,
            detected_language: row.detected_language.map(Language::new),
            created_at: DateTime::from_timestamp(row.created_at, 0).unwrap().to_rfc3339(),
            updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap().to_rfc3339(),
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

    async fn save_job(&self, job: &Job) -> Result<(), StorageError> {
        let config_json = serde_json::to_string(&job.config).unwrap();
        let state_json = serde_json::to_string(&job.state).unwrap();
        let progress_json = serde_json::to_string(&job.progress).unwrap();
        let fidelity_json = serde_json::to_string(&job.fidelity_report).unwrap();
        let export_opts_json = serde_json::to_string(&job.export_options).unwrap();
        let export_report_json = serde_json::to_string(&job.export_report).unwrap();
        let created_at = job.created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let updated_at = job.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let started_at = job.started_at.map(|d| d.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64);
        let completed_at = job.completed_at.map(|d| d.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64);
        let id_str = job.id.to_string();
        let project_id_str = "".to_string(); // placeholder for project_id

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
        let row = sqlx::query!(
            "SELECT id, config, state, progress, fidelity_report, export_options, export_report, created_at, updated_at, started_at, completed_at FROM jobs WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let job = Job {
                id: Uuid::parse_str(&row.id).unwrap().into(),
                config: serde_json::from_str(&row.config).unwrap(),
                state: serde_json::from_str(&row.state).unwrap(),
                progress: serde_json::from_str(&row.progress).unwrap(),
                fidelity_report: serde_json::from_str(&row.fidelity_report).unwrap(),
                export_options: serde_json::from_str(&row.export_options).unwrap(),
                export_report: serde_json::from_str(&row.export_report).unwrap(),
                created_at: DateTime::from_timestamp(row.created_at, 0).unwrap(),
                updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap(),
                started_at: row.started_at.map(|t| DateTime::from_timestamp(t, 0).unwrap()),
                completed_at: row.completed_at.map(|t| DateTime::from_timestamp(t, 0).unwrap()),
            };
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn load_jobs(&self, project_id: &Uuid) -> Result<Vec<JobSummary>, StorageError> {
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
        let section_str = unit.section.to_string();
        let id_str = unit.id.to_string();
        let retry_count = unit.retry_count as i64;

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
        let row = sqlx::query!(
            "SELECT id, source_ref, kind, text, section, state, translation, qa_warnings, retry_count, metadata FROM units WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let unit = Unit {
                id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
                source_ref: serde_json::from_str(&row.source_ref).unwrap(),
                kind: serde_json::from_str(&row.kind).unwrap(),
                text: serde_json::from_str(&row.text).unwrap(),
                section: SectionId(Uuid::parse_str(&row.section).unwrap()),
                state: serde_json::from_str(&row.state).unwrap(),
                translation: row.translation.as_ref().map(|t| serde_json::from_str(t).unwrap()),
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
            source_ref: serde_json::from_str(&row.source_ref).unwrap(),
            kind: serde_json::from_str(&row.kind).unwrap(),
            text: serde_json::from_str(&row.text).unwrap(),
            section: SectionId(Uuid::parse_str(&row.section).unwrap()),
            state: serde_json::from_str(&row.state).unwrap(),
            translation: row.translation.as_ref().map(|t| serde_json::from_str(t).unwrap()),
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
        let created_at = glossary.created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let updated_at = glossary.updated_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let id_str = glossary.id.to_string();
        let project_id_str = glossary.project_id.as_ref().map(|u| u.to_string());
        let job_id_str = glossary.job_id.as_ref().map(|u| u.to_string());

        sqlx::query!(
            r#"
            INSERT INTO glossaries (id, project_id, job_id, name, lang_pair_source, lang_pair_target, entries, metadata, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            id_str,
            project_id_str,
            job_id_str,
            glossary.name,
            glossary.lang_pair.source.code(),
            glossary.lang_pair.target.code(),
            entries_json,
            metadata_json,
            created_at,
            updated_at,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn load_glossary(&self, id: &Uuid) -> Result<Option<Glossary>, StorageError> {
        let row = sqlx::query!(
            "SELECT id, project_id, job_id, name, lang_pair_source, lang_pair_target, entries, metadata, created_at, updated_at FROM glossaries WHERE id = ?",
            id.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        if let Some(row) = row {
            let glossary = Glossary {
                id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
                project_id: row.project_id.map(|s| Uuid::parse_str(&s).unwrap().into()),
                job_id: row.job_id.map(|s| Uuid::parse_str(&s).unwrap().into()),
                name: row.name,
                lang_pair: LangPair { source: Language::new(&row.lang_pair_source), target: Language::new(&row.lang_pair_target) },
                entries: serde_json::from_str(&row.entries).unwrap(),
                metadata: serde_json::from_str(&row.metadata).unwrap(),
                created_at: DateTime::from_timestamp(row.created_at, 0).unwrap(),
                updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap(),
            };
            Ok(Some(glossary))
        } else {
            Ok(None)
        }
    }

    async fn list_glossaries(&self, project_id: &Uuid) -> Result<Vec<Glossary>, StorageError> {
        let rows = sqlx::query!(
            "SELECT id, project_id, job_id, name, lang_pair_source, lang_pair_target, entries, metadata, created_at, updated_at FROM glossaries WHERE project_id = ?",
            project_id.to_string()
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| Glossary {
            id: Uuid::parse_str(row.id.as_ref().unwrap()).unwrap().into(),
            project_id: row.project_id.map(|s| Uuid::parse_str(&s).unwrap().into()),
            job_id: row.job_id.map(|s| Uuid::parse_str(&s).unwrap().into()),
            name: row.name,
            lang_pair: LangPair { source: Language::new(&row.lang_pair_source), target: Language::new(&row.lang_pair_target) },
            entries: serde_json::from_str(&row.entries).unwrap(),
            metadata: serde_json::from_str(&row.metadata).unwrap(),
            created_at: DateTime::from_timestamp(row.created_at, 0).unwrap(),
            updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap(),
        }).collect())
    }

    async fn add_tm_entry(&self, entry: &pgl_domain::TranslationMemoryEntry) -> Result<(), StorageError> {
        let context_json = entry.context.as_ref().map(|c| serde_json::to_string(c).unwrap());
        let source_lang = entry.source_lang.code();
        let target_lang = entry.target_lang.code();

        sqlx::query!(
            r#"
            INSERT INTO translation_memory (source_text, target_text, source_lang, target_lang, context, glossary_hash, quality_score)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(source_text, target_text, source_lang, target_lang, glossary_hash) DO UPDATE SET
                target_text = excluded.target_text,
                context = excluded.context,
                quality_score = excluded.quality_score,
                usage_count = usage_count + 1,
                updated_at = strftime('%s', 'now')
            "#,
            entry.source_text,
            entry.target_text,
            source_lang,
            target_lang,
            context_json,
            entry.glossary_hash,
            entry.quality_score,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;
        Ok(())
    }

    async fn search_tm(&self, source_lang: &Language, target_lang: &Language, query: &str, limit: usize) -> Result<Vec<pgl_domain::TranslationMemoryEntry>, StorageError> {
        let rows = sqlx::query!(
            "SELECT source_text, target_text, source_lang, target_lang, context, glossary_hash, quality_score, usage_count, created_at, updated_at FROM translation_memory WHERE source_lang = ? AND target_lang = ? AND source_text LIKE ? ORDER BY quality_score DESC LIMIT ?",
            source_lang.code(),
            target_lang.code(),
            format!("%{}%", query),
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Sqlx(e.into()))?;

        Ok(rows.into_iter().map(|row| pgl_domain::TranslationMemoryEntry {
            source_text: row.source_text,
            target_text: row.target_text,
            source_lang: Language::new(&row.source_lang),
            target_lang: Language::new(&row.target_lang),
            context: row.context.as_ref().map(|s| serde_json::from_str(s).unwrap()),
            glossary_hash: row.glossary_hash,
            quality_score: row.quality_score,
            usage_count: row.usage_count,
            created_at: DateTime::from_timestamp(row.created_at, 0).unwrap(),
            updated_at: DateTime::from_timestamp(row.updated_at, 0).unwrap(),
        }).collect())
    }

    async fn save_asset(&self, asset: &AssetRef) -> Result<(), StorageError> {
        // Placeholder
        Ok(())
    }

    async fn get_asset(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        // Placeholder
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

    async fn save_job(&self, job: &Job) -> Result<(), StorageError> {
        self.save_job(job).await
    }

    async fn load_job(&self, id: &Uuid) -> Result<Option<Job>, StorageError> {
        self.load_job(id).await
    }

    async fn load_jobs(&self, project_id: &Uuid) -> Result<Vec<JobSummary>, StorageError> {
        self.list_jobs(project_id).await
    }

    async fn update_job(&self, job: &Job) -> Result<(), StorageError> {
        self.save_job(job).await
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

    async fn save_glossary(&self, glossary: &Glossary) -> Result<(), StorageError> {
        self.save_glossary(glossary).await
    }

    async fn load_glossary(&self, id: &Uuid) -> Result<Option<Glossary>, StorageError> {
        self.load_glossary(id).await
    }

    async fn list_glossaries(&self, project_id: &Uuid) -> Result<Vec<Glossary>, StorageError> {
        self.list_glossaries(project_id).await
    }

    async fn add_tm_entry(&self, entry: &pgl_domain::TranslationMemoryEntry) -> Result<(), StorageError> {
        self.add_tm_entry(entry).await
    }

    async fn search_tm(&self, source_lang: &Language, target_lang: &Language, query: &str, limit: usize) -> Result<Vec<pgl_domain::TranslationMemoryEntry>, StorageError> {
        self.search_tm(source_lang, target_lang, query, limit).await
    }

    async fn save_asset(&self, asset: &AssetRef) -> Result<(), StorageError> {
        self.save_asset(asset).await
    }

    async fn get_asset(&self, id: &str) -> Result<Option<Vec<u8>>, StorageError> {
        self.get_asset(id).await
    }
}