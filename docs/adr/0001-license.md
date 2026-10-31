# ADR-0001: Licença do Projeto

**Status:** Proposto

## Contexto

O Polyglottus precisa de uma licença definida desde o início. A escolha afeta:
- Adoção pela comunidade
- Compatibilidade com dependências
- Possibilidade de forks comerciais
- Distribuição no Flathub

## Decisão

Licença **MIT OR Apache-2.0** (dual licensing).

## Consequências

**Positivas:**
- Máxima compatibilidade com dependências (todas permissivas)
- Permite uso comercial e forks fechados (maior adoção)
- Aceita no Flathub sem restrições
- Simples e bem entendida

**Negativas:**
- Não impede forks proprietários (diferente de GPL/AGPL)
- Menos proteção para contribuições da comunidade

## Alternativas Consideradas

1. **GPL-3.0 / AGPL-3.0** (usada pelo TBL) - Força código aberto, mas incompatível com algumas dependências e uso corporativo
2. **Apache-2.0** apenas - Boa, mas MIT é mais simples
3. **MPL-2.0** - Equilíbrio, mas mais complexa

## Verificação

- [x] Todas as dependências diretas são MIT/Apache-2.0/BSD/ISC
- [x] `cargo deny` configurado para bloquear GPL/AGPL
- [x] Flathub aceita MIT/Apache-2.0