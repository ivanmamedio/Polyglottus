<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  export let onNavigate: (view: string, projectId?: string) => void;
  export let currentView: string;
  export let selectedProject: string | null;

  const dispatch = createEventDispatcher();

  async function importBook() {
    try {
      const result = await invoke<{ id: string; name: string }>('import_book', {
        file_path: '',
        project_name: null,
      });
      // This will be called with a real file path from the file picker
      dispatch('import');
    } catch (e) {
      console.error('Import failed:', e);
    }
  }
</script>

<header class="bg-white border-b border-gray-200">
  <div class="flex items-center justify-between px-6 py-4">
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 bg-indigo-600 rounded-lg flex items-center justify-center">
        <svg class="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
      </div>
      <h1 class="text-xl font-bold text-gray-900">Polyglottus</h1>
    </div>

    <div class="flex items-center gap-4">
      {#if currentView === 'list'}
        <button
          onclick={() => onNavigate('import')}
          class="px-4 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 transition flex items-center gap-2"
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
          Import Book
        </button>
      {/if}

      {#if selectedProject}
        <button
          onclick={() => onNavigate('list')}
          class="px-4 py-2 bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200 transition"
        >
          Back to Projects
        </button>
      {/if}

      <button
        onclick={() => onNavigate('settings')}
        class={currentView === 'settings'
          ? 'px-4 py-2 bg-indigo-600 text-white rounded-md'
          : 'px-4 py-2 bg-gray-100 text-gray-700 rounded-md hover:bg-gray-200 transition'}
      >
        Settings
      </button>
    </div>
  </div>
</header>
