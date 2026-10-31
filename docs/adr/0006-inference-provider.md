# ADR-0006: Inferência atrás de Porta (TranslationProvider)

**Status:** Aceito

## Contexto

Como integrar múltiplos motores de inferência (llama.cpp, Ollama, OpenAI API, servidores locais) de forma intercambiável?

## Decisão

**Uma porta `TranslationProvider` com múltiplos adaptadores**, cada um em processo separado quando necessário.

### Arquitetura

```
App (pgl-app)
    |
    ├── TranslationProvider (trait)
    |       |
    |       ├── pgl-provider-openai    → HTTP OpenAI-compat (llama-server, LM Studio, vLLM, OpenAI, etc.)
    |       ├── pgl-provider-ollama    → HTTP Ollama API nativa
    |       ├── pgl-provider-embedded  → llama.cpp via pgl-engine (processo separado)
    |       └── pgl-provider-mt        → NLLB/MADLAD via ONNX/CTranslate2 (Fase 3)
    |
    └── QaChecker (domínio puro) → Valida saída de qualquer provider
```

### Processo Separado para Embedded (llama.cpp)

**Por que processo separado (`pgl-engine`)?**
- Isola falhas (crash do llama.cpp não derruba a UI)
- Isola memória GPU (VRAM não vaza para processo principal)
- Permite reiniciar motor sem reiniciar app
- Fixar versão do `llama-cpp-2` (não segue semver)
- Comunicação via stdin/stdout JSON-RPC ou gRPC

### Capabilities

Cada provider expõe `Capabilities`:
- `max_context_tokens`
- `supported_languages`
- `is_local` (avisa usuário se dados saem da máquina)
- `supports_streaming`
- `supports_glossary`
- `default_model`

### Roteamento por Idioma (Fase 3)

```rust
fn select_provider(lang_pair: &LangPair) -> Arc<dyn TranslationProvider> {
    // Se MT dedicada suporta o par -> usa (melhor para idiomas de poucos recursos)
    // Senao -> usa LLM geral (OpenAI-compat/Ollama/Embedded)
}
```

## Consequências

**Positivas:**
- Troca de motor sem tocar no pipeline
- UI mostra "Local" vs "API" claramente
- Usuario traz sua propria chave (sem custo de infra)
- Testavel com `FakeProvider` (deterministico, sem GPU/rede)

**Negativas:**
- Mais complexidade de setup (multiplos binarios)
- `llama-cpp-2` sem semver requer pinning rigoroso
- Comunicacao inter-processo adiciona latencia