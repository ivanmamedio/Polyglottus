<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  const dispatch = createEventDispatcher();

  let apiKeys: Record<string, string> = {};
  let loading = false;
  let saving = false;
  let message: string | null = null;

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
  let hardware: any = null;

  async function loadSettings() {
    loading = true;
    try {
      const settings = await invoke<any>('get_settings');
      console.log('Settings:', settings);
    } catch (e) {
      console.error('Failed to load settings', e);
    } finally {
      loading = false;
    }
  }

  async function loadModels() {
    try {
      models = await invoke<ModelInfo[]>('list_models');
    } catch (e) {
      console.error('Failed to load models', e);
    }
  }

  async function loadHardware() {
    try {
      hardware = await invoke<any>('recommend_model', { target_lang: 'pt-BR' });
    } catch (e) {
      console.error('Failed to load hardware', e);
    }
  }

  async function setApiKey(provider: string, key: string) {
    saving = true;
    try {
      await invoke('set_api_key', { provider, key });
      apiKeys[provider] = key;
      message = `API key saved for ${provider}`;
      setTimeout(() => message = null, 3000);
    } catch (e) {
      message = 'Failed to save API key';
      setTimeout(() => message = null, 3000);
      console.error(e);
    } finally {
      saving = false;
    }
  }

  onMount(async () => {
    await loadSettings();
    await loadModels();
    await loadHardware();
  });
</script>

<div class="space-y-6">
  <h1 class="text-xl font-bold text-gray-900">Settings</h1>

  {#if message}
    <div class="p-4 bg-blue-50 border border-blue-200 rounded-md text-blue-800 text-sm">
      {message}
    </div>
  {/if}

  <div class="border border-gray-200 rounded-lg p-6">
    <h2 class="text-lg font-semibold text-gray-900 mb-6">API Keys</h2>
    <div class="space-y-4">
      {['openai', 'ollama', 'embedded'] as provider}
      <div class="flex items-center justify-between p-4 border border-gray-200 rounded-lg">
        <div>
          <h3 class="text-sm font-medium text-gray-900 capitalize">{provider}</h3>
          <p class="text-xs text-gray-500">API key for {provider} provider</p>
        </div>
        <div class="flex items-center gap-3">
          <input
            bind:value={apiKeys[provider] || ''}
            type="password"
            placeholder="Enter API key"
            class="w-64 p-2 border border-gray-300 rounded-md text-sm"
          />
          <button
            onclick={() => setApiKey(provider, apiKeys[provider] || '')}
            disabled={saving}
            class="px-4 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 disabled:opacity-50 transition text-sm"
          >
            Save
          </button>
        </div>
      </div>
      {/each}
    </div>
  </div>

  <div class="border border-gray-200 rounded-lg p-6">
    <h2 class="text-lg font-semibold text-gray-900 mb-6">Models</h2>
    <div class="space-y-4">
      {#if models.length === 0}
        <p class="text-gray-500 text-sm">No models installed.</p>
      {/else}
        <table class="w-full text-left text-sm">
          <thead>
            <tr class="bg-gray-50">
              <th class="px-4 py-3 font-medium text-gray-600">Model</th>
              <th class="px-4 py-3 font-medium text-gray-600">Size</th>
              <th class="px-4 py-3 font-medium text-gray-600">Local</th>
              <th class="px-4 py-3 font-medium text-gray-600">License</th>
            </tr>
          </thead>
          <tbody>
            {#each models as model}
              <tr class="border-b border-gray-100">
                <td class="px-4 py-3">{model.name}</td>
                <td class="px-4 py-3">{model.size_gb?.toFixed(1) ?? 'N/A'} GB</td>
                <td class="px-4 py-3">{model.is_local ? 'Yes' : 'No'}</td>
                <td class="px-4 py-3">{model.license ?? 'N/A'}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>

  <div class="border border-gray-200 rounded-lg p-6">
    <h2 class="text-lg font-semibold text-gray-900 mb-6">Hardware Profile</h2>
    {#if hardware}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
        <div class="p-4 bg-gray-50 rounded-lg">
          <h4 class="text-xs font-semibold text-gray-500 uppercase mb-2">CPU Cores</h4>
          <p class="text-gray-900">{hardware.hardware_profile?.cpu_cores ?? 'N/A'}</p>
        </div>
        <div class="p-4 bg-gray-50 rounded-lg">
          <h4 class="text-xs font-semibold text-gray-500 uppercase mb-2">RAM</h4>
          <p class="text-gray-900">{hardware.hardware_profile?.ram_gb?.toFixed(1) ?? 'N/A'} GB</p>
        </div>
        <div class="p-4 bg-gray-50 rounded-lg">
          <h4 class="text-xs font-semibold text-gray-500 uppercase mb-2">Platform</h4>
          <p class="text-gray-900">{hardware.hardware_profile?.platform ?? 'N/A'}</p>
        </div>
        <div class="p-4 bg-gray-50 rounded-lg">
          <h4 class="text-xs font-semibold text-gray-500 uppercase mb-2">Arch</h4>
          <p class="text-gray-900">{hardware.hardware_profile?.arch ?? 'N/A'}</p>
        </div>
      </div>
    {:else}
      <p class="text-gray-500 text-sm">Hardware detection not available.</p>
    {/if}
  </div>
</div>
