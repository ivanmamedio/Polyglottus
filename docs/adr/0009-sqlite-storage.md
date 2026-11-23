# ADR-0009: SQLite como Armazenamento (Projetos, Jobs, Unidades, Glossário, TM)

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

O Polyglottus precisa persistir:
- **Projetos**: metadados, arquivo fonte, formato detectado, fidelidade
- **Jobs**: configuração, estado, progresso, unidades, fidelidade
- **Unidades**: texto original, tradução, estado, warnings QA, retry count
- **Glossários**: entradas, metadados, associação projeto/job
- **Memória de Tradução (TM)**: cache por hash(texto + lang_pair + glossário)
- **Histórico**: eventos, logs, estatísticas

Requisitos:
- **Local-first**: arquivo único, sem servidor
- **Concorrência**: UI + CLI + jobs background acessam simultaneamente
- **Retomada**: checkpoint por unidade — sobrevive a crash/desligamento
- **Streaming**: livro grande (1000+ páginas) não cabe na RAM
- **Portabilidade**: Windows + Linux (Flatpak) + futuro mobile
- **Backup simples**: copiar arquivo `.db`

## Decisão

**SQLite via `sqlx` + `rusqlite`** como storage engine único.

- **Arquivo único**: `polyglottus.db` no diretório de dados do app
- **WAL mode**: `PRAGMA journal_mode=WAL` — leitores concorrentes, escritor único
- **Schema versionado**: `sqlx::migrate!` — migrações embutidas no binário
- **Repositório**: `SqliteRepository` implementa `ProjectRepository` trait

## Schema Principal

```sql
-- Projetos
CREATE TABLE projects (
    id TEXT PRIMARY KEY,           -- UUID v7
    name TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_format TEXT NOT NULL,
    detected_language TEXT,
    fidelity_report TEXT,          -- JSON
    export_options TEXT,           -- JSON
    global_glossary TEXT,          -- JSON (Glossary)
    global_style_guide TEXT,       -- JSON (StyleGuide)
    created_at INTEGER NOT NULL,   -- Unix timestamp
    updated_at INTEGER NOT NULL,
    metadata TEXT                  -- JSON extensível
);

-- Jobs
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    config TEXT NOT NULL,          -- JSON (JobConfig)
    state TEXT NOT NULL,           -- JSON (JobState)
    progress TEXT NOT NULL,        -- JSON (JobProgress)
    fidelity_report TEXT,          -- JSON
    export_options TEXT,           -- JSON
    export_report TEXT,            -- JSON
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER
);

-- Unidades (uma linha por unidade — checkpoint granular)
CREATE TABLE units (
    id TEXT PRIMARY KEY,           -- UnitId (UUID v7)
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    source_ref TEXT NOT NULL,      -- JSON (SourceRef)
    kind TEXT NOT NULL,            -- JSON (UnitKind)
    text TEXT NOT NULL,            -- JSON (MarkedText)
    section TEXT NOT NULL,         -- SectionId (UUID)
    state TEXT NOT NULL,           -- JSON (UnitState)
    translation TEXT,              -- JSON (MarkedText) NULL se pendente
    qa_warnings TEXT NOT NULL,     -- JSON (Vec<QaWarning>)
    retry_count INTEGER DEFAULT 0,
    metadata TEXT                  -- JSON extensível
);

-- Índices para queries comuns
CREATE INDEX idx_units_job_state ON units(job_id, state);
CREATE INDEX idx_units_section ON units(section);

-- Glossários
CREATE TABLE glossaries (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    job_id TEXT REFERENCES jobs(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    lang_pair_source TEXT NOT NULL,
    lang_pair_target TEXT NOT NULL,
    entries TEXT NOT NULL,         -- JSON (HashMap<String, GlossaryEntry>)
    metadata TEXT,                 -- JSON (GlossaryMetadata)
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Memória de Tradução (TM) — cache global por hash
CREATE TABLE translation_memory (
    hash TEXT PRIMARY KEY,         -- BLAKE3(source_text + lang_pair + glossary_hash)
    source_text TEXT NOT NULL,
    target_text TEXT NOT NULL,     -- MarkedText JSON
    lang_pair_source TEXT NOT NULL,
    lang_pair_target TEXT NOT NULL,
    glossary_hash TEXT,            -- Hash do glossário usado (para invalidação)
    created_at INTEGER NOT NULL,
    hit_count INTEGER DEFAULT 1,
    last_used_at INTEGER NOT NULL
);

CREATE INDEX idx_tm_lang ON translation_memory(lang_pair_source, lang_pair_target);
```

## Concorrência e Retomada

| Cenário | Como SQLite resolve |
|---------|---------------------|
| UI lê progresso enquanto job escreve unidades | **WAL mode** — leitores não bloqueiam escritor |
| App fecha durante tradução | Unidades `Pending` permanecem — `Job::can_resume()` = true |
| Crash de energia | Transação SQLite atômica — unidade ou toda salva, ou nada |
| Duas instâncias abrem mesmo DB | `sqlite3` locking — segunda instância falha graciosamente (tratar no app) |

## Por que NÃO JSON/Arquivos

| Critério | SQLite | JSON/Arquivos |
|----------|--------|---------------|
| Checkpoint por unidade | ✅ `UPDATE units SET state=...` | ❌ Reescrever arquivo inteiro |
| Query "unidades pendentes do job X" | ✅ `SELECT ... WHERE job_id=? AND state='Pending'` | ❌ Carregar tudo, filtrar em memória |
| Concorrência UI + background | ✅ WAL | ❌ Lock manual, race conditions |
| Migração schema | ✅ `sqlx::migrate` | ❌ Manual, frágil |
| Backup | ✅ Copiar 1 arquivo | ⚠️ Muitos arquivos, consistência? |
| Tamanho livro 1000+ páginas | ✅ Streaming, índice | ❌ RAM explode |

## TM (Translation Memory) — Design

- **Chave**: `BLAKE3(source_text || lang_pair || glossary_hash)` — 256 bits → hex string
- **Valor**: `MarkedText` traduzido + contadores de uso
- **Invalidação**: Se glossário muda → `glossary_hash` novo → entrada TM separada (não invalida antiga, apenas cria nova)
- **Import/Export**: TMX padrão (XML) — `Glossary::to_tmx()` + parser TMX futuro

## Consequências

- ✅ **Arquivo único** portável, backup trivial
- ✅ **Checkpoint granular** — retomada por unidade nativa
- ✅ **Streaming natural** — unidades fluem do DB, nunca livro inteiro na RAM
- ✅ **Concorrência segura** — WAL mode
- ✅ **Migrações versionadas** — `sqlx::migrate!` embutido
- ✅ **TM global** — reaproveita traduções entre projetos/jobs
- ⚠️ **Flatpak sandbox**: DB em `XDG_DATA_HOME/polyglottus/polyglottus.db` (acesso via portal ou diretório app)
- ⚠️ **Migrações destrutivas**: Evitar `DROP COLUMN` — só `ADD COLUMN` + defaults

## Verificação

- `crates/adapters/pgl-storage/src/lib.rs`: `SqliteRepository` implementa `ProjectRepository` completo
- Migrações em `crates/adapters/pgl-storage/migrations/` (embutidas via `sqlx::migrate!`)
- `WAL mode` ativado na conexão (`PRAGMA journal_mode=WAL`)
- Testes de concorrência: UI lê progresso enquanto `ExecuteJob` escreve unidades
- `cargo test --workspace --test corpus` valida pipeline com DB real