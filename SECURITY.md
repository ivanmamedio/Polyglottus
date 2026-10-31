# Política de Segurança

## Versões Suportadas

| Versão | Suportada |
|--------|-----------|
| 0.x.x  | ❌ (pré-1.0, breaking changes frequentes) |
| ≥ 1.0  | ✅ (última versão estável) |

## Reportando Vulnerabilidades

**NÃO** abra issues públicos para vulnerabilidades de segurança.

Envie um email para **security@polyglottus.org** com:

1. Descrição da vulnerabilidade
2. Passos para reproduzir (se aplicável)
3. Impacto potencial
4. Versão afetada
5. Seu nome/handle para crédito (opcional)

Responderemos em até **72 horas** e coordenaremos a correção e divulgação responsável.

## Práticas de Segurança

### O que o Polyglottus NÃO faz

- ❌ Telemetria obrigatória (opt-in apenas, local por padrão)
- ❌ Envia dados para servidores sem consentimento explícito
- ❌ Armazena chaves de API em arquivos de texto (usa cofre do sistema)
- ❌ Remove DRM de arquivos
- ❌ Executa código dos arquivos traduzidos

### Proteções Implementadas

- **Entradas hostis:** limites de descompressão (zip bomb), anti-zip-slip, XML seguro (sem entidades externas)
- **Webview endurecida:** CSP estrita, permissões mínimas Tauri, sem conteúdo remoto na UI
- **Aviso explícito** quando API externa é escolhida: "o texto será enviado ao provedor X"
- **Chaves de API** no Credential Manager (Windows) / Secret Service (Linux) via `keyring`
- **Sandbox Flatpak** com portais de arquivos, sem `--filesystem=home` desnecessário
- **Fuzzing** contínuo nos parsers (ZIP, XML, PDF) via `cargo-fuzz`
- **Dependências auditadas** no CI (`cargo-audit`, `cargo-deny`)

### Modelo de Ameaça

| Ameaça | Mitigação |
|--------|-----------|
| Arquivo malicioso (zip bomb, zip slip) | Limites de tamanho/ratio, validação de caminhos |
| XXE / XML externo | Parser XML sem resolução de entidades |
| Injeção na UI | CSP, sanitização, sem `eval`/script remoto |
| Vazamento de chave API | Cofre do sistema, nunca em disco/logs |
| Dados sensíveis em logs | `tracing` com filtros, sem serialização de segredos |
| Dependência vulnerável | `cargo-audit` no CI, `cargo-deny` para licenças/bans |

## Divulgação Responsável

1. Recebimento do relatório → confirmação em 72h
2. Análise e reprodução → correção em branch privada
3. Teste da correção → preparação de release
4. Divulgação coordenada (CVE se aplicável) + release público
5. Crédito ao relator (se desejado)

## Hall of Fame

Agradecemos aos pesquisadores que reportaram vulnerabilidades responsavelmente:

<!-- Adicionar nomes após primeiros reports -->

---

**Obrigado por ajudar a manter o Polyglottus seguro!** 🔒