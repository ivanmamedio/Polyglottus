# ADR-0010: Distribuição — Instalador Windows (MSI) + Flatpak (Flathub)

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

Plataformas alvo:
- **Windows 10/11** (x64, ARM64 futuro)
- **Linux via Flatpak** (Flathub) — roda em qualquer distro com Flatpak

Requisitos:
- Instalador "de verdade" (next-next-finish, sem terminal, sem Python)
- Auto-atualização (Windows: MSI + GitHub Releases; Flatpak: Flathub)
- Assinatura de código Windows (SmartScreen)
- Build reprodutível e offline (Flathub)
- Tamanho mínimo (núcleo leve, pacotes opcionais separados)

## Decisão

| Plataforma | Formato | Ferramenta | CI |
|------------|---------|------------|----|
| **Windows** | `.msi` (Windows Installer) | `tauri-cli` + `wix` | `package-windows` (windows-latest) |
| **Linux** | `.flatpak` (Flathub) | `flatpak-builder` + manifesto | `package-flatpak` (ubuntu-latest) |

**AppImage / .deb / .rpm / Snap**: **Não no MVP** — Flatpak cobre Linux universalmente.

## Windows — MSI via Tauri

```toml
# apps/desktop/src-tauri/tauri.conf.json
{
  "bundle": {
    "windows": {
      "wix": {
        "language": "pt-BR",
        "licenseFile": "LICENSE",
        "fragmentPaths": ["packaging/windows/wix-fragments"]
      },
      "nsis": null  -- Não usar NSIS; MSI é padrão enterprise
    }
  }
}
```

**Dependências Windows:**
- WebView2 Runtime (já no Win 10 1803+ / Win 11)
- Visual C++ Redistributable (bundlado pelo Tauri via `cargo tauri build`)

**Assinatura de Código (SmartScreen):**
| Opção | Status |
|-------|--------|
| Certificado EV (DigiCert, Sectigo, etc.) | **Recomendado** — remove aviso SmartScreen imediato |
| Certificado OV padrão | Remove aviso após reputação |
| **Sem certificado** | ⚠️ SmartScreen avisa "Windows protegeu seu PC" — documentar na página de download |
| GitHub Actions `azure/code-sign` | Se certificado Azure Key Vault |

**Política:** Lançar **sem certificado inicialmente** (custo zero), documentar aviso SmartScreen, buscar patrocínio/certificado comunitário depois.

## Linux — Flatpak (Flathub)

### Por que Flatpak?
- **Universal**: Roda em Fedora, Ubuntu, Arch, Debian, openSUSE, etc.
- **Sandbox**: Isolamento por padrão (portal de arquivos, rede, GPU)
- **Runtime GNOME**: WebKitGTK + dependências Tauri já incluídas
- **Flathub**: Loja central, auto-update, descoberta
- **Build offline**: Fontes Cargo + npm geradas localmente → `flatpak-builder` offline

### Manifesto (`packaging/flatpak/org.polyglottus.Polyglottus.yml`)

```yaml
app-id: org.polyglottus.Polyglottus
runtime: org.gnome.Platform
runtime-version: '47'
sdk: org.gnome.Sdk
command: polyglottus
finish-args:
  - --share=network              # Para Ollama/llama-server local
  - --device=dri                 # GPU (Vulkan/VA-API)
  - --filesystem=xdg-documents   # Portal de arquivos (padrão)
  - --talk-name=org.freedesktop.secrets  # Keyring (Secret Service)
  - --env=POLARS_MAX_THREADS=1   # Evita oversubscription
build-options:
  env:
    CARGO_HOME: /run/build/polyglottus/cargo
    RUSTUP_TOOLCHAIN: stable
modules:
  - name: polyglottus
    buildsystem: simple
    build-commands:
      - cargo install --path apps/desktop/src-tauri --locked --root /app
    sources:
      - type: dir
        path: .
    # Fontes offline geradas por flatpak-builder-tools:
    - generated-sources.json  # cargo-sources + node-sources
```

### Fontes Offline (Obrigatório Flathub)

```bash
# Gerar uma vez (ou no CI antes do build)
cargo install cargo-cargo-sources
flatpak-builder-tools/cargo/flatpak-cargo-generator.py Cargo.lock -o generated-sources.json
flatpak-builder-tools/npm/flatpak-npm-generator.py apps/desktop/ui/package-lock.json -o generated-npm-sources.json
```

**CI `package-flatpak` faz isso automaticamente.**

### Extensões Opcionais (Pacotes Pesados)

| Extensão | Conteúdo | Tamanho | Instalação |
|----------|----------|---------|------------|
| `org.polyglottus.Polyglottus.Pandoc` | Pandoc ~150MB | Opcional | `flatpak install flathub org.polyglottus.Polyglottus.Pandoc` |
| `org.polyglottus.Polyglottus.Tesseract` | Tesseract + dados idioma | Opcional | Idem |
| `org.polyglottus.Polyglottus.Fonts.CJK` | Noto Sans CJK ~500MB | Opcional | Idem |
| `org.polyglottus.Polyglottus.LlamaServer` | llama-server binário | Opcional | Idem |

**App detecta extensões** via `--filesystem=xdg-run/flatpak/org.polyglottus.Polyglottus.*` e oferece no Settings.

## Estrutura de Pacotes

```
packaging/
├── flatpak/
│   ├── org.polyglottus.Polyglottus.yml          # App base
│   ├── org.polyglottus.Polyglottus.Pandoc.yml   # Extension Pandoc
│   ├── org.polyglottus.Polyglottus.Tesseract.yml
│   ├── org.polyglottus.Polyglottus.Fonts.CJK.yml
│   ├── metainfo.xml                             # AppStream (screenshots, descrição)
│   └── polyglottus.desktop                      # .desktop entry
├── windows/
│   ├── wix-fragments/                           # WiX customizações (UI pt-BR, license)
│   └── sign.ps1                                 # Script assinatura (se certificado)
└── scripts/
    ├── generate-flatpak-sources.sh
    └── verify-checksums.sh
```

## CI/CD — Matriz de Build

```yaml
# .github/workflows/ci.yml
package-windows:
  runs-on: windows-latest
  if: github.ref == 'refs/heads/main'
  steps:
    - cargo tauri build --target x86_64-pc-windows-msvc
    - Upload MSI artifact

package-flatpak:
  runs-on: ubuntu-latest
  if: github.ref == 'refs/heads/main'
  steps:
    - flatpak-builder --force-clean build-dir org.polyglottus.Polyglottus.yml
    - Upload .flatpak artifact

release:
  needs: [package-windows, package-flatpak]
  if: startsWith(github.ref, 'refs/tags/v')
  steps:
    - Download artifacts
    - gh release create --generate-notes
    - Checksums SHA256 no corpo da release
```

## Consequências

- ✅ **Windows**: MSI nativo, integração Start Menu, desinstalação limpa, enterprise-ready
- ✅ **Linux**: Flatpak universal, sandbox, auto-update Flathub, runtime GNOME compartilhado
- ✅ **Build reprodutível**: Fontes offline, `Cargo.lock` + `package-lock.json` fixos
- ✅ **Pacotes opcionais**: Pandoc/Tesseract/Fonts/CJK como extensions — app base ~20MB
- ⚠️ **SmartScreen sem certificado**: Avisar usuários na página de download
- ⚠️ **Flatpak sandbox**: Portal de arquivos obrigatório (não `--filesystem=home`); testar cedo
- ⚠️ **GPU no Flatpak**: `--device=dri` + extensões NVIDIA/AMD — CPU sempre funciona
- ⚠️ **Ollama no Flatpak**: `--share=network` permite localhost; alternativa: extension `LlamaServer`

## Verificação

- `apps/desktop/src-tauri/tauri.conf.json`: `bundle.windows.wix` configurado
- `packaging/flatpak/org.polyglottus.Polyglottus.yml`: manifesto base + `metainfo.xml` + `.desktop`
- CI `.github/workflows/ci.yml`: jobs `package-windows` + `package-flatpak` + `release`
- `flatpak-builder-tools` geram `generated-sources.json` + `generated-npm-sources.json` no CI
- `cargo deny check` valida licenças de todas as dependências nativas