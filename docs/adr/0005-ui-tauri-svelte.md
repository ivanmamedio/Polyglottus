# ADR-0005: UI - Tauri + Svelte

**Status:** Aceito

## Contexto

Escolha de tecnologia para UI desktop.

## Decisão

**Tauri + Svelte 5 + TypeScript + Tailwind CSS**

## Alternativas Consideradas

| Opção | Prós | Contras |
|-------|------|---------|
| **Tauri + Svelte** (escolhido) | Leve (webview do sistema), UI rica, ecossistema grande, bundle pequeno | Duas linguagens, webview difere (WebView2/WebKitGTK) |
| Tauri + React | Mais conhecido | Bundle/runtime maiores |
| Slint/Iced/egui (100% Rust) | Uma linguagem, binário único | Edição rica, acessibilidade, virtualização mais difíceis |
| Electron | Ecossistema maduro | Pesado (Chromium embutido), contradiz princípio "leve" |

## Consequências

**Positivas:**
- Instalador pequeno (~10-20 MB base)
- Webview do sistema (WebView2 no Windows, WebKitGTK no Linux)
- Svelte 5: reatividade fine-grained, bundle pequeno
- TypeScript: tipos compartilhados com Rust via `specta`/`ts-rs`
- Tailwind: design system consistente, dark mode nativo
- Acessibilidade: Svelte + HTML semântico + ARIA

**Negativas:**
- Duas stacks (Rust + JS/TS)
- WebView2 precisa estar instalado no Windows (padrão no Win10/11)
- WebKitGTK no Linux precisa de dependências do sistema
- CSP restrita no Tauri limita alguns padrões JS

## Regras de Ouro

1. **Núcleo é biblioteca headless** (`pgl-app` + `pgl-domain`) com CLI (`pgl-cli`)
2. **UI é só mais um adaptador** - pode ser trocada no futuro
3. **Tipos compartilhados** gerados do Rust (`specta`/`ts-rs`) → zero divergência IPC/TS
4. **Sem lógica de negócio na UI** - só apresentação e comandos Tauri

## Stack Detalhada

- **Frontend:** Svelte 5, TypeScript 5, Tailwind CSS 3
- **Build:** Vite 5
- **IPC:** Tauri 2 commands + events
- **State:** Svelte stores / runes
- **i18n:** svelte-i18n ou similar (pt-BR, en, es mínimo)
- **Testing:** Vitest (unit), Playwright (e2e)