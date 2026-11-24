<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  export let onSelect: (view: string, projectId?: string) => void;

  interface ProjectSummary {
    id: string;
    name: string;
    source_file: string;
    source_format: string;
    created_at: string;
    updated_at: string;
    overall_progress: number;
  }

  let projects: ProjectSummary[] = [];
  let loading = false;
  let error: string | null = null;

  async function loadProjects() {
    loading = true;
    error = null;
    try {
      projects = await invoke<ProjectSummary[]>('list_projects');
    } catch (e) {
      error = 'Failed to load projects';
      console.error(e);
    } finally {
      loading = false;
    }
  }

  async function deleteProject(id: string) {
    if (!confirm('Delete this project?')) return;
    try {
      await invoke('delete_project', { id });
      projects = projects.filter(p => p.id !== id);
    } catch (e) {
      error = 'Failed to delete project';
      console.error(e);
    }
  }

  function formatProgress(p: number): string {
    return `${p}%`;
  }

  onMount(loadProjects);
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-gray-900">Recent Projects</h2>
    <button
      onclick={() => onSelect('import')}
      class="px-4 py-2 bg-indigo-600 text-white rounded-md hover:bg-indigo-700 transition text-sm"
    >
      + New Project
    </button>
  </div>

  {#if loading}
    <div class="py-8 text-center text-gray-500">Loading projects...</div>
  {:else if error}
    <div class="p-4 bg-red-50 border border-red-200 rounded-md text-red-800 text-sm">
      {error}
    </div>
  {:else if projects.length === 0}
    <div class="py-12 text-center">
      <div class="w-12 h-12 bg-gray-200 rounded-full flex items-center justify-center mx-auto mb-3">
        <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
        </svg>
      </div>
      <h3 class="text-lg font-medium text-gray-900">No projects yet</h3>
      <p class="text-gray-500 mt-2">Import your first book to get started.</p>
    </div>
  {:else}
    <div class="grid gap-4">
      {#each projects as project}
        <div class="border border-gray-200 rounded-lg p-4 hover:border-indigo-300 transition cursor-pointer group"
               onclick={() => onSelect('detail', project.id)}>
          <div class="flex items-start justify-between mb-2">
            <h3 class="text-sm font-semibold text-gray-900 group-hover:text-indigo-600">{project.name}</h3>
            <span class="text-xs px-2 py-1 bg-gray-100 text-gray-600 rounded">{project.source_format}</span>
          </div>
          <div class="flex items-center gap-4 text-xs text-gray-500 mb-3">
            <span>{project.source_file.split('/').pop() ?? project.source_file.split('\\').pop()}</span>
            <span>•</span>
            <span>{new Date(project.updated_at).toLocaleDateString()}</span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-2 mb-2">
            <div
              class="bg-indigo-600 h-2 rounded-full transition-all duration-500"
              style="width: {project.overall_progress}%"
            ></div>
          </div>
          <div class="flex items-center gap-2 text-xs text-gray-500">
            <span>{formatProgress(project.overall_progress)} completed</span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
