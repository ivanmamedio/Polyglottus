# ADR-0003: Arquitetura Hexagonal

**Status:** Aceito

## Contexto

O projeto precisa ser:
- Testável sem modelo (FakeProvider)
- Extensível (novos formatos, provedores)
- Mantível a longo prazo
- Separado de detalhes de infraestrutura

## Decisão

Arquitetura **Hexagonal (Ports & Adapters)** com três camadas:

```
Domain (pgl-domain)          → Tipos puros, regras, sem I/O, sem async
    ↑
App (pgl-app)                → Use cases + Ports (traits)
    ↑
Adapters (crates/adapters/*) → Implementações concretas
```

**Regras:**
- Domain não conhece App, Adapters, Tauri, SQLite, HTTP, etc.
- App só conhece Domain e Ports (traits)
- Adapters implementam Ports e são injetados no binário (DI manual)
- Validação de grafo de dependências no CI (`cargo-deny`)

## Consequências

**Positivas:**
- Testes unitários rápidos (domain puro)
- Testes de integração com FakeProvider (sem GPU/rede)
- Fácil adicionar formato/provedor (um crate, um registro)
- Troca de UI/Storage/Provider sem tocar no domínio

**Negativas:**
- Mais boilerplate inicial (traits, DI)
- Curva de aprendizado para contribuidores
- Mais crates para gerenciar

## Implementação

- `pgl-domain`: Zero dependências externas, `#![forbid(unsafe_code)]`
- `pgl-app`: Define traits `FormatReader`, `TranslationProvider`, `OcrEngine`, `ProjectRepository`, `SecretStore`, `ModelCatalog`, `EventSink`
- Adapters: Um crate por formato/provedor implementando as traits
- Binários (`pgl-cli`, `apps/desktop/src-tauri`): Fazem a injeção de dependências