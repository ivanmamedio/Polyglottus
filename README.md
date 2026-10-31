# Polyglottus

> **Tradutor de livros e documentos, gratuito, open source e *local-first*.**
> Plataformas: **Windows** e **Linux (Flatpak)**.

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tauri](https://img.shields.io/badge/tauri-2.0-blueviolet.svg)](https://tauri.app)
[![CI](https://github.com/polyglottus/polyglottus/workflows/CI/badge.svg)](https://github.com/polyglottus/polyglottus/actions)

---

## ✨ Visão Geral

O **Polyglottus** traduz livros e documentos inteiros, no maior número possível de formatos (EPUB, PDF, DOCX e muitos outros), preservando estrutura, imagens e o máximo possível do layout.

### Princípios

1. **Gratuito e open source** — sem taxa, sem conta, sem telemetria obrigatória
2. **Local-first** — o livro nunca sai da sua máquina, a menos que você escolha uma API externa
3. **Rust-first** — núcleo em Rust, binários nativos isolados atrás de interfaces
4. **Leve** — instalador pequeno, pouca RAM, abertura rápida, sem Python/Electron no núcleo
5. **Honesto sobre fidelidade** — cada formato tem um nível de fidelidade declarado e visível

---

## 📦 Formatos Suportados

| Fase | Formatos de Entrada | Saída | Fidelidade |
|------|---------------------|-------|------------|
| **1 (MVP)** | EPUB, HTML, TXT, Markdown, SRT/VTT | Mesmo + Bilíngue | ★★★★★ |
| **2** | DOCX, PPTX, XLSX, ODT/ODP/ODS, FB2, PO/XLIFF/TMX | Mesmo + Bilíngue | ★★★★★ |
| **3** | PDF (texto), MOBI, AZW3, RTF, DOC, LaTeX (via Pandoc) | EPUB/DOCX/HTML | ★★★☆☆ |
| **4** | PDF escaneado, CBZ/CBR | EPUB/DOCX/HTML | ★★☆☆☆ |

> 📖 **PDF com texto:** modo *reflow* (exporta como EPUB/DOCX) → modo *overlay* (mantém PDF) → modo *re-diagramação* (pesquisa)

---

## 🏗️ Arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│  Adaptadores de Entrada:  UI Tauri  │  CLI  │  (futuro: API)   │
└────────────────────────────────────┬────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Aplicação (pgl-app): ImportarLivro · PlanejarTradução         │
│  ExecutarJob · RevisarUnidade · Exportar · ...                  │
│  Define PORTAS (traits) que o mundo externo implementa          │
└────────────────────────────────────┬────────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Domínio (pgl-domain): Unit, MarkedText, Glossary, StyleGuide, │
│  Project, JobState, QA, Segmentação, Placeholders (sem I/O)    │
└────────────────────────────────────┬────────────────────────────┘
                                     │
┌────────────────────────────────────▼────────────────────────────┐
│  Adaptadores de Saída: Formatos · Provedores · OCR · Storage   │
│  Segredos · Modelos · Logs                                      │
└─────────────────────────────────────────────────────────────────┘
```

- **Hexagonal / Portas e Adaptadores** — domínio puro, testável sem modelo
- **Unidades de tradução** — streaming natural, checkpoint por unidade, retomada automática
- **Uma porta, vários adaptadores** — OpenAI-compat, Ollama, llama.cpp embutido, MT dedicada (NLLB)

---

## 🚀 Início Rápido

### Pré-requisitos

- **Rust** 1.78+ (`rustup install stable`)
- **Node.js** 20+ e **pnpm** (para UI Tauri)
- **Linux:** `webkit2gtk-4.1`, `libssl-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`
- **Windows:** Visual Studio Build Tools, WebView2 Runtime

### Desenvolvimento

```bash
# Clone o repositório
git clone https://github.com/polyglottus/polyglottus.git
cd polyglottus

# Instale dependências e configure
just setup

# Rode a UI em modo desenvolvimento
just run-desktop

# Ou rode a CLI
just run-cli -- --help
```

### Comandos Úteis

```bash
just check          # Verificação de tipos rápida
just lint           # Clippy + fmt
just test           # Todos os testes (nextest)
just build          # Build release de tudo
just ci             # Pipeline CI completo local
just doc            # Gerar documentação
```

---

## 📁 Estrutura do Repositório

```
polyglottus/
├─ Cargo.toml                    # Workspace Rust
├─ rust-toolchain.toml           # Toolchain fixa
├─ justfile                      # Comandos de dev
├─ crates/
│  ├─ domain/   pgl-domain/      # Domínio puro (sem I/O, sem async)
│  ├─ app/      pgl-app/         # Casos de uso + Portas (traits)
│  ├─ adapters/
│  │  ├─ formats/                # Leitores/escritores por formato
│  │  ├─ providers/              # Provedores de tradução
│  │  ├─ pgl-ocr/                # OCR (ocrs + Tesseract opcional)
│  │  ├─ pgl-storage/            # SQLite (unidades, estado, TM)
│  │  ├─ pgl-secrets/            # Cofre do sistema (keyring)
│  │  └─ pgl-models/             # Catálogo, hardware, download
│  ├─ bins/
│  │  ├─ pgl-cli/                # CLI (testes E2E)
│  │  └─ pgl-engine/             # Processo isolado de inferência
│  └─ testing/ pgl-testkit/      # FakeProvider, fixtures, helpers
├─ apps/
│  └─ desktop/                   # App Tauri
│     ├─ src-tauri/              # Shell Rust: comandos, eventos, DI
│     └─ ui/                     # Front-end (Svelte + TypeScript)
├─ packaging/
│  ├─ flatpak/                   # Manifesto Flatpak (Flathub)
│  └─ windows/                   # Configuração instalador Windows
├─ test-corpus/                  # Livros de domínio público + casos difíceis
├─ docs/                         # ADRs, arquitetura, formatos, contribuição
└─ .github/workflows/            # CI/CD
```

---

## 🧪 Testes

```bash
# Testes unitários (domínio puro, fake provider)
just test-unit

# Testes de integração (pipeline completo com FakeProvider)
just test-integration

# Testes de snapshot (formatos contra test-corpus)
cargo insta test --workspace --accept

# Fuzzing (leitores de ZIP/XML/PDF)
cargo fuzz run <target>

# Benchmarks
just bench
```

> **FakeProvider** — tradutor determinístico (inverte/marca texto) que valida o pipeline inteiro sem GPU, rede ou modelo real.

---

## 📋 Roadmap

| Fase | Foco | Entregável Principal |
|------|------|----------------------|
| **0** | Fundação | Repo, CI, ADRs, spikes (Tauri Flatpak/Windows, patch EPUB/DOCX) |
| **1** | MVP | EPUB/HTML/TXT/SRT + OpenAI-compat/Ollama + UI + CLI |
| **2** | Escritório + Qualidade | DOCX/PPTX/XLSX/ODF/FB2/PO + Revisão lado a lado + TM + OCR |
| **3** | PDF + Cauda Longa | PDF reflow/overlay + Pandoc + MT dedicada + llama.cpp embutido |
| **4** | Avançado | PDF escaneado, re-diagramação, CBZ, inpainting, plugins |

Veja [ROADMAP.md](docs/roadmap.md) e [Polyglottus-Planning.md](Polyglottus-Planning.md) para detalhes.

---

## 🤝 Contribuindo

Contribuições são bem-vindas! Leia [CONTRIBUTING.md](CONTRIBUTING.md) para:

- Como adicionar um formato (crate `pgl-fmt-xxx`)
- Como adicionar um provedor (crate `pgl-provider-xxx`)
- Padrões de código, commits, PRs
- *Good first issues* rotuladas

---

## 📄 Licença

**MIT OR Apache-2.0** — veja [LICENSE](LICENSE) para detalhes.

> A licença final será decidida na ADR-0001 após auditoria de dependências.

---

## 🔗 Links

- **Documentação:** [docs/](docs/)
- **Planejamento completo:** [Polyglottus-Planning.md](Polyglottus-Planning.md)
- **Issues:** [GitHub Issues](https://github.com/polyglottus/polyglottus/issues)
- **Discussões:** [GitHub Discussions](https://github.com/polyglottus/polyglottus/discussions)

---

*Feito com ❤️ pela comunidade Polyglottus*