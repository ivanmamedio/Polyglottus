# ADR-0001: Licença do Projeto

**Status:** Aceito
**Data:** 2026-11-23

## Contexto

O Polyglottus é um tradutor de livros e documentos, gratuito, open source e local-first. A escolha da licença afeta:
- Adoção pela comunidade e empresas
- Compatibilidade com dependências
- Capacidade de impedir forks fechados (copyleft vs permissiva)

## Decisão

**Licença dual: MIT OR Apache-2.0**

## Justificativa

1. **Máxima adoção:** Licenças permissivas permitem uso em projetos proprietários, corporativos e comerciais sem obrigações virais.

2. **Compatibilidade com dependências:** Auditoria das dependências atuais (Cargo.toml):
   - `pgl-domain`, `pgl-app`, todos crates internos: MIT/Apache-2.0
   - `lopdf`, `docx-rs`, `ocrs`, `llama-cpp-2`: MIT ou Apache-2.0
   - `pdfium-render`: BSD-3/Apache-2.0
   - `tauri`: MIT/Apache-2.0
   - `sqlx`, `rusqlite`: MIT/Apache-2.0
   - `keyring`: MIT/Apache-2.0
   - **Nenhuma dependência GPL/AGPL** no núcleo (evitado `PyMuPDF`/MuPDF que é AGPL)

3. **Dual licensing (MIT OR Apache-2.0):**
   - Apache-2.0 traz grant de patentes explícito
   - MIT é mais simples e amplamente reconhecida
   - Usuário escolhe a que preferir

4. **Decisão explícita contra copyleft (GPL/AGPL):**
   - O projeto concorrente TBL usa AGPL-3.0
   - Nossa estratégia é **diferenciar por leveza, Rust-first, cobertura de formatos** — não por restrição de licença
   - Queremos que empresas possam usar, contribuir e distribuir sem medo

## Consequências

- ✅ Uso livre em qualquer contexto (comercial, proprietário, SaaS)
- ✅ Contribuições da comunidade simplificadas (CLA trivial)
- ✅ Compatível com todas as dependências atuais
- ⚠️ Forks fechados são permitidos (aceitável — nossa vantagem é técnica, não legal)
- ⚠️ Se no futuro adicionarmos dependência GPL, reavaliar

## Verificação

- `cargo deny check licenses` passa no CI
- Arquivo `LICENSE` na raiz contém ambos os textos
- Cabeçalho de cada crate: `license = "MIT OR Apache-2.0"`