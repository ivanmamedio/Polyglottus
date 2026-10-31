# ADR-0010: Distribuição - Windows Installer + Flatpak

**Status:** Aceito

## Contexto

Plataformas alvo: **Windows** e **Linux via Flatpak (Flathub)**.

## Decisão

### Windows

- **Tauri built-in installer** (MSI via WiX)
- **Dependência**: WebView2 (presente no Windows 10/11 atualizados)
- **Assinatura de código**: Sem certificado → SmartScreen avisa
  - Opções: comprar certificado EV (~$300-600/ano), ou documentar aviso
  - Open source: pode usar GitHub Actions + `signtool` com certificado gratuito (ex: SignPath.io para OSS)
- **GPU**: CUDA/Vulkan variam por driver; CPU sempre funciona como fallback
- **Caminhos**: Suporte a Unicode e caminhos longos (`\\?\` prefix)

### Linux (Flatpak)

- **Não é alvo de build do Tauri**: Gera `.deb` primeiro → `flatpak-builder` a partir de manifesto
- **Runtime**: GNOME 23.08 (traz WebKitGTK, libs corretas)
- **Build offline**: `cargo-sources.json` + `node-sources.json` via `flatpak-builder-tools`
- **Sandbox**: Portais de arquivos (seletor do sistema), sem `--filesystem=home` desnecessário
- **Ollama/servidor local**: Rede local via `--talk-name=org.freedesktop.portals.*` ou empacotar `llama-server` como extensão
- **GPU**: Extensões NVIDIA/AMD/Vulkan; testar cedo; CPU sempre funciona
- **Pacotes opcionais**: Pandoc, Tesseract, fontes como **extensões Flatpak** (a confirmar)
- **Cofre de chaves**: Secret Service via portal
- **Armazenamento modelos**: Pasta de dados do app (`XDG_DATA_HOME`), opção de apontar para outro disco
- **Fontes**: Acesso às fontes do sistema para idiomas de destino

### CI de Plataforma

Matriz GitHub Actions:
- Windows 10/11 (MSVC)
- Linux: Fedora 39+ e Ubuntu 22.04+ (Flatpak)

## Consequências

**Positivas:**
- Duas plataformas cobertas com ferramentas nativas
- Flatpak no Flathub = descoberta, atualizações automáticas, sandbox
- Windows MSI = instalação familiar, registro de desinstalação
- Build reprodutível (fontes fixadas)

**Negativas:**
- Dois pipelines de build/pacote
- Flatpak tem curva de aprendizado (manifesto, runtime, portais)
- Assinatura Windows custa ou requer workaround
- GPU no Flatpak/Linux é complexo