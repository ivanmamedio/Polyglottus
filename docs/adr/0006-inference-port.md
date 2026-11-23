# ADR-0006: Inferência Atrás de Porta `TranslationProvider` + Processo Isolado

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

O Polyglottus suporta múltiplos backends de inferência:
- **OpenAI-compatível** (HTTP): llama-server, LM Studio, vLLM, LocalAI, OpenAI API, etc.
- **Ollama** (API nativa): modelos já instalados localmente
- **Motor embutido** (`llama.cpp`): "funciona sem instalar nada" — experiência zero-config
- **MT dedicada** (NLLB, MADLAD): idiomas de poucos recursos (Fase 3)

Riscos do `llama.cpp` in-process:
- Crashes derrubam a UI inteira (segfault em C++)
- Vazamento de memória GPU (VRAM não liberada)
- Bloqueio da thread principal durante inferência
- `llama-cpp-2` **não segue semver** — quebras em atualização

## Decisão

1. **Porta única `TranslationProvider`** (trait em `pgl-app`) — todos backends implementam
2. **Motor embutido (`llama.cpp`) roda em processo separado** (`pgl-engine` binário)
3. **IPC entre `EmbeddedProvider` (in-process) ↔ `pgl-engine` (isolado)**
4. **Fixar versão do `llama-cpp-2`** no `Cargo.lock` + `deny.toml`

## Arquitetura

```
┌─────────────────────────────────────────────────────────────┐
│                    Processo Principal                        │
│  pgl-app (ExecuteJob)                                        │
│       │                                                      │
│       ▼  TranslationProvider trait                           │
│  ┌─────────────────────┐                                     │
│  │ EmbeddedProvider    │  (adapter in-process)              │
│  └──────────┬──────────┘                                     │
└─────────────┼────────────────────────────────────────────────┘
              │ IPC (JSON-RPC over stdin/stdout ou Unix socket)
              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Processo Isolado (pgl-engine)             │
│  llama-cpp-2 + llama.cpp (C++)                               │
│  - Carrega modelo GGUF                                       │
│  - Gerencia KV cache, VRAM                                   │
│  - Health check, graceful shutdown                           │
│  - Reinicia automaticamente se crashar                       │
└─────────────────────────────────────────────────────────────┘
```

## Porta `TranslationProvider` (em `pgl-app`)

```rust
#[async_trait]
pub trait TranslationProvider: Send + Sync + 'static {
    fn capabilities(&self) -> Capabilities;
    async fn translate(&self, req: TranslateRequest) -> Result<TranslateResponse, ProviderError>;
    async fn health_check(&self) -> Result<(), ProviderError> { Ok(()) }
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> { Ok(vec![]) }
}
```

**Implementações:**
| Provedor | Crate | Processo | Caso de Uso |
|----------|-------|----------|-------------|
| `OpenAICompatProvider` | `pgl-provider-openai` | In-process (HTTP client) | llama-server, LM Studio, APIs |
| `OllamaProvider` | `pgl-provider-ollama` | In-process (HTTP client) | Ollama local |
| `EmbeddedProvider` | `pgl-provider-embedded` | **IPC → pgl-engine** | Zero-config, offline |
| `MtProvider` (futuro) | `pgl-provider-mt` | In-process (ONNX via `ort`) | NLLB/MADLAD |

## `pgl-engine` — Binário Isolado

- **Crate:** `crates/bins/pgl-engine` (habilitado no workspace)
- **Entrada:** CLI args (model path, GPU layers, threads, ctx size) + JSON-RPC stdin/stdout
- **Protocolo:** Request/Response + notificações (progresso, logs)
- **Health:** Ping/pong + auto-restart supervisionado pelo `EmbeddedProvider`
- **Crash isolation:** Se `pgl-engine` morre, `EmbeddedProvider` detecta (stdin fechado), relança, reconecta — job retoma do checkpoint (SQLite)

## Versionamento `llama-cpp-2`

```toml
# Cargo.toml [workspace.dependencies]
llama-cpp-2 = { version = "0.1.160", features = [] }  # FIXO

# deny.toml
[deny]
bans = [
  { name = "llama-cpp-2", version = "=0.1.160" }  # Bloqueia atualização automática
]
```

**Política:** Atualização manual apenas após:
1. Testes de regressão com modelos conhecidos (TranslateGemma, Qwen, Mistral)
2. Verificação de breaking changes no changelog upstream
3. Atualização coordenada de `pgl-engine` + `pgl-provider-embedded`

## Consequências

- ✅ **Isolamento de falhas**: Crash no `llama.cpp` não derruba UI
- ✅ **Gerenciamento VRAM**: Processo dedicado libera GPU ao terminar
- ✅ **Troca de motor**: `EmbeddedProvider` ↔ `OpenAICompatProvider` (llama-server) transparente para `ExecuteJob`
- ✅ **Version pinning**: `llama-cpp-2` não quebra build silenciosamente
- ✅ **Testabilidade**: `FakeProvider` substitui qualquer backend em testes
- ⚠️ **Latência IPC**: ~1-2ms overhead por batch (aceitável vs batch de 10 unidades)
- ⚠️ **Complexidade**: Dois binários para distribuir (mitigado: Tauri bundla ambos)

## Verificação

- `crates/bins/pgl-engine/Cargo.toml` existe e compila
- `pgl-provider-embedded/src/lib.rs` implementa `TranslationProvider` + IPC client
- CI `package-windows` + `package-flatpak` inclui `pgl-engine` no bundle
- `cargo deny check` bloqueia upgrade de `llama-cpp-2`