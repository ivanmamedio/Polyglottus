# Guia de Contribuição

Obrigado por querer contribuir com o Polyglottus! 🎉

## Índice

1. [Como Começar](#como-começar)
2. [Padrões de Código](#padrões-de-código)
3. [Commits e PRs](#commits-e-prs)
4. [Adicionando um Formato](#adicionando-um-formato)
5. [Adicionando um Provedor de Tradução](#adicionando-um-provedor-de-tradução)
6. [Testes](#testes)
7. [Reportando Bugs](#reportando-bugs)
8. [Sugerindo Funcionalidades](#sugerindo-funcionalidades)

---

## Como Começar

```bash
# 1. Fork e clone
git clone https://github.com/seu-usuario/polyglottus.git
cd polyglottus

# 2. Setup do ambiente
just setup

# 3. Crie uma branch
git checkout -b minha-contribuicao

# 4. Faça suas alterações
# ...

# 5. Rode os checks
just pre-commit

# 6. Commit e push
git add .
git commit -m "feat: minha contribuição"
git push origin minha-contribuicao

# 7. Abra um PR
```

---

## Padrões de Código

### Rust

- **Edição 2021**, `rustfmt` + `clippy` (config no repo)
- **Arquitetura hexagonal** — domínio puro (`pgl-domain`) sem I/O, sem `async`
- **Erros tipados** — `thiserror` nas libs, `anyhow` só nos binários
- **Zero `unwrap()`/`expect()`** fora de testes
- **`#![forbid(unsafe_code)]`** em todos os crates (exceto adaptadores FFI)
- **Observabilidade** — `tracing` com spans por job/unidade
- **Novos tipos** — `UnitId`, `SectionId`, `Lang`, etc. em vez de `String`/`u64` genéricos

### Front-end (Tauri UI)

- **Svelte 5 + TypeScript** (ou Solid)
- **Tokens de design** em `shared/styles`
- **Componentes pequenos** e reutilizáveis
- **Acessibilidade** (WCAG AA, navegação por teclado, leitores de tela)
- **i18n** — pt-BR, en, es no mínimo

---

## Commits e PRs

### Conventional Commits

```
<tipo>(<escopo>): <descrição curta>

[corpo opcional]

[rodapé opcional]
```

**Tipos:**
- `feat` — nova funcionalidade
- `fix` — correção de bug
- `docs` — documentação
- `style` — formatação (sem mudança de lógica)
- `refactor` — refatoração
- `perf` — performance
- `test` — testes
- `chore` — manutenção (deps, build, etc.)
- `ci` — CI/CD

**Exemplos:**
```
feat(formats): add EPUB reader with patch-in-place
fix(ocr): handle low-confidence results gracefully
docs(adr): add ADR-0003 hexagonal architecture
```

### Pull Requests

- **Pequenos e focados** — uma mudança lógica por PR
- **Título claro** seguindo Conventional Commits
- **Descrição** com contexto, motivação e como testar
- **Testes** — adicione testes para novas funcionalidades
- **Documentação** — atualize docs se necessário
- **CI verde** — todos os checks devem passar

---

## Adicionando um Formato

1. Crie um crate em `crates/adapters/formats/pgl-fmt-<nome>/`
2. Implemente `FormatReader` e/ou `FormatWriter` (traits em `pgl-app`)
3. Registre o formato em **um único lugar** (factory em `pgl-app`)
4. Adicione testes de snapshot com `insta` usando `test-corpus/`
5. Atualize a matriz de formatos em `docs/formats/matrix.md`

### Estrutura Mínima do Crate

```
pgl-fmt-meuformato/
├─ Cargo.toml
├─ src/
│  ├─ lib.rs          # public API: reader/writer structs
│  ├─ reader.rs       # impl FormatReader
│  ├─ writer.rs       # impl FormatWriter
│  ├─ sniff.rs        # detecção de formato (magic bytes, extensão)
│  └─ fidelity.rs     # relatório de fidelidade do formato
└─ tests/
   ├─ integration.rs  # testes com arquivos reais
   └─ snapshots/      # insta snapshots
```

---

## Adicionando um Provedor de Tradução

1. Crie um crate em `crates/adapters/providers/pgl-provider-<nome>/`
2. Implemente `TranslationProvider` (trait em `pgl-app`)
3. Suporte a `capabilities()` (contexto máx, idiomas, local/remoto)
4. Trate `TranslateRequest` → `TranslateResponse` com streaming se possível
5. Adicione testes de contrato via `pgl-testkit`

---

## Testes

```bash
# Todos os testes
just test

# Apenas unitários (rápido)
just test-unit

# Integração (pipeline com FakeProvider)
just test-integration

# Snapshots (aceitar mudanças)
cargo insta test --workspace --accept

# Fuzzing (parsers)
cargo fuzz run <target>

# Cobertura
just coverage
```

### Pirâmide de Testes

| Nível | O que testa | Ferramenta |
|-------|-------------|------------|
| Unitário | Domínio: placeholders, QA, segmentação, glossário | `cargo test`, `proptest` |
| Snapshot | Leitores/escritores contra arquivos reais | `insta` + `test-corpus/` |
| Contrato | Todo adaptador passa na mesma suíte | `pgl-testkit` |
| Integração | Pipeline inteiro com `FakeProvider` | `pgl-testkit`, `pgl-cli` |
| Fuzzing | Parsers contra entradas hostis | `cargo-fuzz` |
| Desempenho | Tempo/memória em livros grandes | `criterion` |
| Validação | EPUB válido, DOCX abre sem reparo | EPUBCheck, verificações OOXML |

---

## Reportando Bugs

Use o template de issue **Bug Report** e inclua:

1. **Versão** do Polyglottus (`pgl-cli --version`)
2. **SO** e arquitetura
3. **Passos para reproduzir** (mínimos)
4. **Arquivo de exemplo** (se possível, domínio público)
5. **Logs** com `RUST_LOG=debug`
6. **Comportamento esperado** vs **atual**

---

## Sugerindo Funcionalidades

Use o template **Feature Request** e descreva:

1. **Problema** que resolve
2. **Solução proposta**
3. **Alternativas** consideradas
4. **Impacto** em arquitetura, performance, UX
5. **Disposição para implementar** (ou apenas sugerir)

---

## Código de Conduta

Este projeto segue o [Contributor Covenant](CODE_OF_CONDUCT.md). Ao participar, você concorda em manter um ambiente respeitoso e inclusivo.

---

## Dúvidas?

- Abra uma **Discussion** no GitHub
- Consulte a [documentação](docs/)
- Veja os [ADRs](docs/adr/) para decisões arquiteturais

---

**Obrigado por contribuir!** 🚀