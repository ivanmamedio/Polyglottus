<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface ModelInfo {
    id: string;
    name: string;
    languages: string[];
    max_context: number;
    is_local: boolean;
    size_gb: number | null;
    license: string | null;
  }

  let models: ModelInfo[] = [];
  let loading = true;
  let activeTab = 'models';

  async function loadModels() {
    try {
      const result = await invoke<ModelInfo[]>('list_models');
      models = result;
    } catch (e) {
      console.error('Failed to load models:', e);
    } finally {
      loading = false;
    }
  }

  async function downloadModel(id: string) {
    try {
      await invoke('download_model', { modelId: id });
      await loadModels();
    } catch (e) {
      alert('Erro ao baixar: ' + e);
    }
  }

  async function removeModel(id: string) {
    if (!confirm('Remover modelo ' + id + '?')) return;
    try {
      await invoke('remove_model', { modelId: id });
      await loadModels();
    } catch (e) {
      alert('Erro ao remover: ' + e);
    }
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-surface-900 dark:text-surface-100">Modelos</h1>
  </div>

  <div class="card">
    <div class="p-4 border-b border-surface-200 dark:border-surface-700">
      <div class="flex gap-4 border-b border-surface-200 dark:border-surface-700">
        <button class={['pb-2 text-sm font-medium', activeTab === 'models' ? 'text-primary-600 border-b-2 border-primary-600' : 'text-surface-500']} onclick={() => activeTab = 'models'}>
          Instalados
        </button>
        <button class={['pb-2 text-sm font-medium', activeTab === 'catalog' ? 'text-primary-600 border-b-2 border-primary-600' : 'text-surface-500']} onclick={() => activeTab = 'catalog'}>
          Catálogo
        </button>
        <button class={['pb-2 text-sm font-medium', activeTab === 'hardware' ? 'text-primary-600 border-b-2 border-primary-600' : 'text-surface-500']} onclick={() => activeTab = 'hardware'}>
          Hardware
        </button>
      </div>
    </div>

    {#if activeTab === 'models'}
      <div class="p-4">
        {#if loading}
          <div class="flex items-center justify-center py-12">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
          </div>
        {:else if models.length === 0}
          <div class="text-center py-12">
            <svg class="mx-auto h-16 w-16 text-surface-300 dark:text-surface-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" />
            </svg>
            <h3 class="mt-4 text-lg font-medium text-surface-900 dark:text-surface-100">Nenhum modelo instalado</h3>
            <p class="mt-2 text-surface-500 dark:text-surface-400">Baixe um modelo na aba "Catálogo" ou use a recomendação automática</p>
          </div>
        {:else}
          <div class="space-y-3">
            {#each models as model}
              <div class="flex items-center justify-between p-4 bg-surface-50 dark:bg-surface-800/50 rounded-lg">
                <div class="flex-1 min-w-0">
                  <p class="font-medium text-surface-900 dark:text-surface-100 truncate">{model.name}</p>
                  <p class="text-sm text-surface-500 dark:text-surface-400">
                    {model.size_gb ? model.size_gb.toFixed(1) + ' GB' : 'Tamanho desconhecido'} •
                    {model.languages.join(', ')} •
                    {model.max_context.toLocaleString()} tokens
                  </p>
                  {#if model.license}
                    <span class="badge-info mt-1">{model.license}</span>
                  {/if}
                </div>
                <button class="btn-danger ml-4" onclick={() => removeModel(model.id)}>
                  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {:else if activeTab === 'catalog'}
      <div class="p-4">
        <p class="text-surface-500 dark:text-surface-400">Catálogo de modelos disponíveis para download (em desenvolvimento)</p>
        <div class="mt-4 space-y-2">
          <button class="btn-secondary w-full justify-start" onclick={() => invoke('recommend_model', { targetLang: 'pt-BR' }).then(loadModels)}>
            Recomendar para Português (BR)
          </button>
          <button class="btn-secondary w-full justify-start" onclick={() => invoke('recommend_model', { targetLang: 'en' }).then(loadModels)}>
            Recomendar para Inglês
          </button>
          <button class="btn-secondary w-full justify-start" onclick={() => invoke('recommend_model', { targetLang: 'es' }).then(loadModels)}>
            Recomendar para Espanhol
          </button>
        </div>
      </div>
    {:else if activeTab === 'hardware'}
      <div class="p-4">
        <p class="text-surface-500 dark:text-surface-400">Detecção de hardware (em desenvolvimento)</p>
      </div>
    {/if}
  </div>
</div>