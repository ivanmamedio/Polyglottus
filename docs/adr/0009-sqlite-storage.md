# ADR-0009: SQLite como Armazenamento

**Status:** Aceito

## Contexto

Onde guardar projetos, jobs, unidades, glossário, memória de tradução, estado?

Opções:
- Arquivos JSON (simples, mas não escala, sem concorrência, sem queries)
- SQLite (embutido, ACID, queries, WAL mode, trivial de empacotar)
- PostgreSQL/outro server (overkill, não local-first)

## Decisão

**SQLite com `rusqlite`/`sqlx`** como armazenamento principal.

### Esquema Principal

- `projects`: metadados do projeto
- `jobs`: configuração e estado do job
- `units`: unidades de tradução (streaming, checkpoint por unidade)
- `glossaries`: glossários por projeto/job
- `translation_memory`: cache TMX (source_text, target_text, langs, contexto, qualidade)
- `assets`: imagens/fonts (deduplicados por hash SHA-256)
- `model_cache`: modelos baixados
- `settings`: configuração do usuário
- `logs`: logs estruturados (opcional)

### Vantagens do SQLite

- **ACID**: Transações garantem consistência (job + unidades atômicos)
- **WAL mode**: Leitores concorrentes não bloqueiam escritores
- **Streaming**: Unidades carregadas sob demanda, não tudo na RAM
- **Queries**: Filtros por estado, seção, job, etc.
- **Backup**: Arquivo único, cópia atômica
- **Portável**: Mesmo arquivo Windows/Linux
- **Zero config**: Biblioteca embutida

### Modo WAL

```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA cache_size=-32768; -- 32 MB cache
PRAGMA temp_store=MEMORY;
```

### Migrações

- `sqlx::migrate` com arquivos SQL versionados em `migrations/`
- Aplicadas automaticamente na inicialização
- Rollback não suportado (apenas forward)

## Consequências

**Positivas:**
- Retomada garantida (estado por unidade persistido)
- Memória constante (streaming + SQLite)
- Queries flexíveis para UI (filtros, progresso)
- Fácil backup/restauração

**Negativas:**
- Schema migrations requerem cuidado
- Não é ideal para blobs grandes (assets > 10 MB → arquivo separado)
- Concorrência limitada a um escritor