-- Initial schema for Polyglottus SQLite storage

-- Projects table
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_format TEXT NOT NULL,
    detected_language TEXT,
    fidelity_report TEXT, -- JSON
    export_options TEXT NOT NULL, -- JSON
    created_at INTEGER NOT NULL, -- Unix timestamp
    updated_at INTEGER NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}', -- JSON
    global_glossary TEXT, -- JSON
    global_style_guide TEXT -- JSON
);

CREATE INDEX idx_projects_updated_at ON projects(updated_at DESC);
CREATE INDEX idx_projects_source_file ON projects(source_file);

-- Jobs table
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    config TEXT NOT NULL, -- JSON (JobConfig)
    state TEXT NOT NULL, -- JSON (JobState)
    progress TEXT NOT NULL, -- JSON (JobProgress)
    fidelity_report TEXT, -- JSON
    export_options TEXT, -- JSON
    export_report TEXT, -- JSON
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);

CREATE INDEX idx_jobs_project_id ON jobs(project_id);
CREATE INDEX idx_jobs_state ON jobs(state);
CREATE INDEX idx_jobs_created_at ON jobs(created_at DESC);

-- Units table (translation units)
CREATE TABLE units (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    source_ref TEXT NOT NULL, -- JSON (SourceRef)
    kind TEXT NOT NULL, -- JSON (UnitKind)
    text TEXT NOT NULL, -- JSON (MarkedText)
    section TEXT NOT NULL, -- SectionId (UUID string)
    state TEXT NOT NULL, -- JSON (UnitState)
    translation TEXT, -- JSON (MarkedText)
    qa_warnings TEXT NOT NULL DEFAULT '[]', -- JSON array
    retry_count INTEGER NOT NULL DEFAULT 0,
    metadata TEXT NOT NULL DEFAULT '{}', -- JSON
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX idx_units_job_id ON units(job_id);
CREATE INDEX idx_units_state ON units(state);
CREATE INDEX idx_units_section ON units(section);
CREATE INDEX idx_units_job_state ON units(job_id, state);

-- Glossaries table
CREATE TABLE glossaries (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    job_id TEXT REFERENCES jobs(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    lang_pair_source TEXT NOT NULL,
    lang_pair_target TEXT NOT NULL,
    entries TEXT NOT NULL, -- JSON array of GlossaryEntry
    metadata TEXT NOT NULL DEFAULT '{}', -- JSON
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_glossaries_project_id ON glossaries(project_id);
CREATE INDEX idx_glossaries_job_id ON glossaries(job_id);

-- Translation Memory (TMX-style)
CREATE TABLE translation_memory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_text TEXT NOT NULL,
    target_text TEXT NOT NULL,
    source_lang TEXT NOT NULL,
    target_lang TEXT NOT NULL,
    context TEXT, -- JSON (surrounding text, section, etc.)
    glossary_hash TEXT, -- Hash do glossário usado
    quality_score REAL DEFAULT 1.0, -- 0.0 a 1.0
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(source_text, target_text, source_lang, target_lang, glossary_hash)
);

CREATE INDEX idx_tm_source ON translation_memory(source_text, source_lang);
CREATE INDEX idx_tm_target ON translation_memory(target_text, target_lang);
CREATE INDEX idx_tm_langs ON translation_memory(source_lang, target_lang);
CREATE INDEX idx_tm_quality ON translation_memory(quality_score DESC);

-- Assets (imagens, fontes, etc.)
CREATE TABLE assets (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    internal_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    hash TEXT NOT NULL, -- SHA-256
    data BLOB, -- Opcional: armazenar bytes se pequeno
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX idx_assets_project_id ON assets(project_id);
CREATE INDEX idx_assets_hash ON assets(hash);

-- Model cache info
CREATE TABLE model_cache (
    model_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    path TEXT NOT NULL,
    downloaded_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    license TEXT
);

-- Settings / Configuração do usuário
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL, -- JSON
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Logs estruturados (opcional, para debug)
CREATE TABLE logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    level TEXT NOT NULL,
    target TEXT NOT NULL,
    message TEXT NOT NULL,
    job_id TEXT,
    unit_id TEXT
);

CREATE INDEX idx_logs_timestamp ON logs(timestamp DESC);
CREATE INDEX idx_logs_job_id ON logs(job_id);
CREATE INDEX idx_logs_level ON logs(level);

-- Trigger para updated_at automático
CREATE TRIGGER trigger_projects_updated_at
AFTER UPDATE ON projects
BEGIN
    UPDATE projects SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trigger_jobs_updated_at
AFTER UPDATE ON jobs
BEGIN
    UPDATE jobs SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trigger_units_updated_at
AFTER UPDATE ON units
BEGIN
    UPDATE units SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trigger_glossaries_updated_at
AFTER UPDATE ON glossaries
BEGIN
    UPDATE glossaries SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trigger_translation_memory_updated_at
AFTER UPDATE ON translation_memory
BEGIN
    UPDATE translation_memory SET updated_at = strftime('%s', 'now') WHERE id = NEW.id;
END;