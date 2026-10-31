# Como Adicionar Idioma da Interface

> O Polyglottus suporta i18n da UI. Mínimo: **pt-BR, en, es**.

---

## Estrutura

```
apps/desktop/ui/src/
├── i18n/
│   ├── index.ts          # Configuração principal
│   ├── locales/
│   │   ├── pt-BR.json    # Português (Brasil)
│   │   ├── en.json       # Inglês
│   │   ├── es.json       # Espanhol
│   │   └── <novo>.json   # Novo idioma
│   └── utils.ts          # Helpers
```

---

## Passo a Passo

### 1. Crie o Arquivo de Tradução

**`apps/desktop/ui/src/i18n/locales/fr.json`** (exemplo Francês):

```json
{
  "app": {
    "name": "Polyglottus",
    "tagline": "Traducteur de livres et documents local-first"
  },
  "nav": {
    "projects": "Projets",
    "models": "Modèles",
    "settings": "Paramètres"
  },
  "project": {
    "new": "Nouveau projet",
    "import": "Importer un livre",
    "delete": "Supprimer",
    "confirmDelete": "Êtes-vous sûr de vouloir supprimer ce projet ?"
  },
  "translation": {
    "start": "Traduire",
    "pause": "Pause",
    "resume": "Reprendre",
    "cancel": "Annuler",
    "export": "Exporter",
    "provider": "Fournisseur",
    "model": "Modèle",
    "targetLanguage": "Langue cible",
    "outputFormat": "Format de sortie",
    "bilingual": "Bilingue",
    "batchSize": "Taille du lot",
    "maxParallel": "Parallélisme max",
    "progress": "Progression",
    "completed": "Terminé",
    "pending": "En attente",
    "failed": "Échoué",
    "flagged": "Signalé"
  },
  "units": {
    "source": "Original",
    "translation": "Traduction",
    "edit": "Modifier",
    "retranslate": "Retraduire",
    "approve": "Approuver",
    "qaWarnings": "Avertissements QA"
  },
  "models": {
    "installed": "Installés",
    "catalog": "Catalogue",
    "hardware": "Matériel",
    "download": "Télécharger",
    "remove": "Supprimer",
    "recommended": "Recommandé pour votre matériel"
  },
  "settings": {
    "apiKeys": "Clés API",
    "dataDir": "Répertoire de données",
    "language": "Langue de l'interface",
    "theme": "Thème"
  },
  "errors": {
    "generic": "Une erreur s'est produite",
    "importFailed": "Échec de l'importation",
    "translationFailed": "Échec de la traduction",
    "exportFailed": "Échec de l'exportation",
    "modelDownloadFailed": "Échec du téléchargement du modèle"
  },
  "common": {
    "save": "Enregistrer",
    "cancel": "Annuler",
    "delete": "Supprimer",
    "edit": "Modifier",
    "close": "Fermer",
    "loading": "Chargement...",
    "yes": "Oui",
    "no": "Non"
  }
}
```

### 2. Registre o Idioma

**`apps/desktop/ui/src/i18n/index.ts`:**

```typescript
// Adicione ao objeto de locales
import fr from './locales/fr.json';

export const locales = {
  'pt-BR': ptBR,
  en,
  es,
  fr,  // <-- Adicione aqui
} as const;

export type Locale = keyof typeof locales;
export const defaultLocale: Locale = 'pt-BR';
export const supportedLocales: Locale[] = ['pt-BR', 'en', 'es', 'fr']; // <-- Adicione
```

### 3. Atualize Seletor de Idioma

No componente de configurações (`Settings.svelte`):

```svelte
<select bind:value={settings.language}>
  <option value="pt-BR">Português (Brasil)</option>
  <option value="en">English</option>
  <option value="es">Español</option>
  <option value="fr">Français</option>  <!-- Adicione -->
</select>
```

### 4. Teste

```bash
cd apps/desktop/ui
pnpm run dev
# Mude idioma nas configurações e verifique toda a UI
```

---

## Convenções de Tradução

### Chaves

- Use **camelCase** aninhado: `translation.start`, `units.qaWarnings`
- Agrupe por funcionalidade: `app`, `nav`, `project`, `translation`, `units`, `models`, `settings`, `errors`, `common`

### Valores

- **Não use variáveis** nos arquivos JSON (use interpolação no componente)
- **Mantenha consistência** de terminologia (ex: "Traduzir" não "Converter")
- **Escaping**: JSON exige escape de aspas `\"` e quebras `\n`

### Interpolação (no componente)

```svelte
<script>
  import { t } from '$lib/i18n';
  
  // Uso com variáveis
  const message = t('translation.progress', { current: 42, total: 100 });
  // "Traduzindo: 42/100" (definido no JSON como "Traduzindo: {current}/{total}")
</script>

{#t 'translation.eta' { time: formatTime(eta) }}
  <span>ETA: {time}</span>
{/t}
```

---

## Checklist para Novo Idioma

- [ ] Arquivo `locales/<codigo>.json` criado com todas as chaves
- [ ] `locales/index.ts` atualizado
- [ ] Seletor de idioma em `Settings.svelte` atualizado
- [ ] `supportedLocales` atualizado
- [ ] Testado visualmente (todas as telas)
- [ ] Verificado: sem chaves faltando (console não mostra warnings)
- [ ] Revisado por falante nativo (se possível)

---

## Ferramentas

### Extrair Chaves Faltantes

```bash
# Script para comparar chaves entre idiomas
node scripts/check-i18n.js
```

### Validação JSON

```bash
# No CI
cd apps/desktop/ui
pnpm run lint:i18n  # Script customizado
```

---

## Boas Práticas

1. **Contexto**: Adicione comentários no JSON para chaves ambíguas
2. **Pluralização**: Use `i18n` library com suporte a plurais (ex: `t('units.count', { count: 5 })`)
3. **RTL**: Teste com árabe/hebraico se adicionar (`dir="rtl"` no HTML)
4. **Fontes**: Verifique se fontes do sistema suportam o script (CJK, árabe, devanagari)