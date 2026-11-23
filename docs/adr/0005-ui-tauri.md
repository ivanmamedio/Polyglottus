# ADR-0005: UI — Tauri + Svelte (Front-end Leve)

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

O Polyglottus precisa de interface desktop para Windows e Linux (Flatpak). Requisitos:
- Leve (instalador pequeno, RAM baixa, abertura rápida)
- Acessível (WCAG AA, teclado, screen readers, i18n)
- Edição de texto rica (revisão lado a lado, listas virtualizadas 10k+ unidades)
- Sem Electron (pesado, Chromium embutido)
- Código do núcleo em Rust (já decidido)

## Decisão

**Tauri v2 + Svelte 4 + TypeScript** como stack de UI.

## Alternativas Avaliadas

| Opção | Prós | Contras | Veredito |
|-------|------|---------|----------|
| **Tauri + Svelte/Solid** (escolhido) | WebView do sistema (WebView2/WebKitGTK), bundle pequeno (~3-5MB), Rust no backend, ecossistema web maduro | Duas linguagens; WebView difere entre OS | ✅ **Escolhido** |
| Tauri + React | Mais conhecido | Bundle + runtime maiores que Svelte/Solid | ❌ |
| Slint / Iced / egui (100% Rust) | Uma linguagem, binário único, sem WebView | Edição rica, acessibilidade, virtualização de listas enormes menos maduros | ❌ **Reavaliar na Fase 3** |
| Electron + React/Svelte | Ecossistema maduro | ~100MB+, RAM alta, Chromium embutido, viola princípio "leve" | ❌ |
| Flutter | Cross-platform nativo | Dart, não Rust; engine próprio pesado | ❌ |

## Arquitetura UI

```
apps/desktop/
├── src-tauri/           # Rust: comandos Tauri, eventos, montagem DI
│   ├── lib.rs           # Setup AppState, comandos #[tauri::command]
│   └── Cargo.toml       # Dependências: pgl-app, pgl-fmt-*, pgl-providers, pgl-storage, pgl-secrets, pgl-models, pgl-ocr
└── ui/                  # Svelte + TypeScript + Vite
    ├── src/
    │   ├── app/         # Bootstrap, rotas, tema
    │   ├── features/    # import/ setup/ progress/ review/ export/ models/ settings/
    │   ├── shared/      # components/ hooks/ ipc/ i18n/ styles/ (design tokens)
    │   └── tests/       # Vitest + Playwright E2E
    ├── package.json
    └── vite.config.ts
```

## Type-Safe IPC

- **`specta` + `ts-rs`** geram tipos TypeScript a partir das structs Rust (`#[derive(Type, TS)]`)
- Elimina divergência entre comandos Rust e chamadas TypeScript
- Exemplo: `ProjectSummary`, `JobProgressDto`, `UnitDto`, `TranslationPlanDto` — definidos 1x no Rust, usados no Svelte

## Design System

- **Tailwind CSS** + tokens de design em `shared/styles/` (cores, espaçamento, tipografia)
- Componentes pequenos e reutilizáveis (`Button`, `Card`, `Badge`, `Toast`, `Modal`, `Table`, `SplitView`)
- Estados padronizados: loading, empty, error, success
- Tema claro/escuro via CSS variables + `prefers-color-scheme`
- i18n: pt-BR, en, es mínimos — arquivos JSON em `shared/i18n/`

## Telas (Roadmap 15.2)

| Tela | Componente | Status |
|------|------------|--------|
| 1. Início (drag-drop, projetos recentes, selo fidelidade) | `ProjectList` + `DropZone` | ⚠️ Parcial |
| 2. Configuração (idioma, provider, modelo, glossário, estilo) | `TranslateModal` + `ModelSelector` | ⚠️ Básico |
| 3. Estimativa (tempo, RAM, custo) | `PlanTranslation` result view | ✅ |
| 4. Progresso (barra, capítulo, velocidade, ETA, pausar/retomar) | `ProjectDetail` + `ProgressBar` | ✅ |
| 5. **Revisão lado a lado** (original \| tradução, edição inline, QA, atalhos) | **`UnitReview` (SplitView)** | ❌ **AUSENTE — CRÍTICO MVP** |
| 6. Exportação (formatos, preview, relatório qualidade) | `ExportModal` | ⚠️ Básico |
| 7. Modelos (instalados, recomendados, download, licenças) | `ModelsView` | ✅ |
| 8. Configurações (idioma UI, chaves API, pasta dados, diagnóstico) | `Settings` | ✅ |

## Primeiro Uso (Assistente)

1. Detectar hardware (`ModelCatalog::detect_hardware()`)
2. Recomendar modelo (`recommend_model()`)
3. Baixar com progresso (`download_model()` + eventos `model-download-progress`)
4. Traduzir parágrafo de teste (`ExecuteJob` com 1 unidade)
5. Mostrar revisão lado a lado → usuário vê funcionando em < 1 min

## Consequências

- ✅ **Leve**: Instalador ~15-25MB (vs 100MB+ Electron), RAM ociosa ~30-50MB
- ✅ **Acessível**: HTML semântico, ARIA, foco visível, escala de fonte, alto contraste
- ✅ **Núcleo headless**: `pgl-app` + `pgl-domain` testáveis sem UI
- ✅ **Troca futura de UI**: Slint/Iced/egui possível sem reescrever domínio/app
- ⚠️ **Duas linguagens**: Rust + TypeScript (mitigado: IPC type-safe, arquitetura hexagonal)
- ⚠️ **WebView diferenças**: WebView2 (Win) vs WebKitGTK (Linux) — testar em ambas no CI

## Verificação

- `apps/desktop/ui/package.json`: Svelte 4, TypeScript, Tailwind, Vite, Vitest
- `apps/desktop/src-tauri/Cargo.toml`: Tauri 2, plugins dialog/fs/shell/opener/store/sql
- CI: `package-flatpak` (Linux) + `package-windows` (MSI) rodam em matriz
- IPC types gerados: `specta` + `ts-rs` → `../ui/src/bindings/`