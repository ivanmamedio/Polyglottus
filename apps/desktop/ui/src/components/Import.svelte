<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';

  export let onBack: () => void;

  const dispatch = createEventDispatcher();

  let loading = false;
  let imported: boolean = false;
  let projectName: string = '';
  let targetLang: string = 'pt-BR';

  async function selectFile() {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'Documents', extensions: ['epub', 'txt', 'md', 'srt', 'vtt', 'html', 'docx', 'odt', 'pdf'] },
        ],
      });

      if (selected && typeof selected === 'string') {
        await importBook(selected);
      }
    } catch (e) {
      console.error('File selection failed:', e);
    }
  }

  async function importBook(filePath: string) {
    loading = true;
    try {
      await invoke('import_book', {
        file_path: filePath,
        project_name: projectName || null,
      });
      imported = true;
    } catch (e) {
      console.error('Import failed:', e);
      alert('Import failed: ' + e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="max-w-2xl mx-auto">
  <div class="text-center py-12">
    {#if imported}
      <div class="bg-green-50 border border-green-200 rounded-lg p-6 mb-6">
        <div class="w-12 h-12 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-4">
          <svg class="w-6 h-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
        </div>
        <h2 class="text-xl font-semibold text-gray-900">Book imported successfully!</h2>
        <p class="text-gray-600 mt-2">Your project has been created and is ready for translation.</p>
        <button onclick={onBack} class="mt-6 px-6 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 transition">
          Go to Project
        </button>
      </div>
    {:else}
      <div class="border-2 border-dashed border-gray-300 rounded-lg p-12 hover:border-indigo-300 transition">
        <div class="w-16 h-16 bg-indigo-100 rounded-full flex items-center justify-center mx-auto mb-6">
          <svg class="w-8 h-8 text-indigo-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
        </div>
        <h2 class="text-xl font-semibold text-gray-900 mb-3">Import a Book</h2>
        <p class="text-gray-600 mb-8">Select a document to translate. Supported formats: EPUB, TXT, MD, SRT, VTT, HTML, DOCX, ODT, PDF</p>

        <button onclick={selectFile} disabled={loading} class="px-8 py-4 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition flex items-center justify-center gap-3 w-full md:w-auto">
          {loading ? 'Importing...' : 'Choose File'}
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
        </button>

        <div class="mt-8 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">Project Name (optional)</label>
            <input bind:value={projectName} type="text" placeholder="My Book Project" class="w-full p-2 border border-gray-300 rounded-md text-sm focus:ring-2 focus:ring-indigo-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">Target Language</label>
            <select bind:value={targetLang} class="w-full p-2 border border-gray-300 rounded-md text-sm focus:ring-2 focus:ring-indigo-500">
              <option value="pt-BR">Portuguese (Brazil)</option>
              <option value="es">Spanish</option>
              <option value="fr">French</option>
              <option value="de">German</option>
              <option value="en">English</option>
            </select>
          </div>
        </div>

        <p class="mt-6 text-xs text-gray-500">
          The book stays on your machine. No data is sent unless you choose an external API provider.
        </p>
      </div>
    {/if}
  </div>
</div>
