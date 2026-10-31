# Changelog

Todas as mudanças notáveis neste projeto serão documentadas neste arquivo.

O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/),
e este projeto adere a [Semantic Versioning](https://semver.org/lang/pt-BR/).

## [Unreleased]

### Adicionado
- Estrutura inicial do workspace Cargo
- Configurações: rust-toolchain, rustfmt, clippy, deny, editorconfig
- Justfile com comandos de desenvolvimento
- Documentação base: README, LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY

---

## Template para Releases

## [X.Y.Z] - YYYY-MM-DD

### Adicionado
- Novas funcionalidades

### Alterado
- Mudanças em funcionalidades existentes

### Depreciado
- Funcionalidades que serão removidas

### Removido
- Funcionalidades removidas

### Corrigido
- Correções de bugs

### Segurança
- Correções de vulnerabilidades

---

## Convenções

- **Tipos de mudança:** `Adicionado`, `Alterado`, `Depreciado`, `Removido`, `Corrigido`, `Segurança`
- **Escopo opcional:** `feat(core)`, `fix(ui)`, `docs(adr)`, etc.
- **Links para PRs/Issues:** `(#123)` ao final de cada entrada
- **Ordem:** mais recente primeiro dentro de cada seção