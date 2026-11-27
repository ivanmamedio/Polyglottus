<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  export let project: any = null;
  export let units: any[] = [];
  export let loading = false;
  export let error: string | null = null;

  async function loadProject() {
    try {
      project = await invoke<any>('get_project', { id: project?.id || '' });
    } catch (e) {
      error = 'Failed to load project';
      console.error(e);
    }
  }

  async function loadUnits() {
    try {
      units = await invoke<any[]>('list_units', { project_id: project?.id || '' });
    } catch (e) {
      console.error('Failed to load units', e);
    }
  }

  onMount(async () => {
    await loadProject();
    await loadUnits();
  });
</script>

<div class="space-y-6">
  <h1 class="text-xl font-bold text-gray-900">Project Details</h1>

  {#if loading}
    <div class="p-4 bg-yellow-50 border border-yellow-200 rounded-md text-yellow-800 text-sm">
      Loading project...
    </div>
  {/if}

  {#if error}
    <div class="p-4 bg-red-50 border border-red-200 rounded-md text-red-800 text-sm">{error}</div>
  {/if}

  {#if project}
    <div class="border border-gray-200 rounded-lg p-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold text-gray-900">{project.name}</h2>
        <p class="text-sm text-gray-500">{project.source_file}</p>
      </div>
      <p class="text-sm text-gray-500">{project.source_format}</p>
    </div>

    {#if units.length > 0}
      <div class="border border-gray-200 rounded-lg">
        <div class="p-4 border-b border-gray-200">
          <h2 class="text-lg font-semibold text-gray-900">Units ({units.length})</h2>
        </div>
        <div class="overflow-x-auto">
          <table class="w-full text-left text-sm">
            <thead class="bg-gray-50">
              <tr>
                <th class="px-4 py-3 font-medium text-gray-600">Source</th>
                <th class="px-4 py-3 font-medium text-gray-600">Translation</th>
                <th class="px-4 py-3 font-medium text-gray-600">State</th>
              </tr>
            </thead>
            <tbody>
              {#each units as unit}
                <tr class="border-b border-gray-100 hover:bg-gray-50">
                  <td class="px-4 py-3 text-gray-700 max-w-xs truncate">{unit.source_text}</td>
                  <td class="px-4 py-3">{unit.translated_text ?? '—'}</td>
                  <td class="px-4 py-3">
                    <span class="text-xs font-semibold uppercase">{unit.state}</span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
    {/if}
  {/if}
</div>
