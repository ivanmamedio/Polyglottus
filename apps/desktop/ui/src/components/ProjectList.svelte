<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  export let onNavigate: (view: 'list' | 'detail' | 'settings', projectId?: string) => void;

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
  let loading = true;
  let error: string | null = null;

  async function loadProjects() {
    try {
      loading = true;
      const result = await invoke<ProjectSummary[]>('list_projects');
      projects = result;
    } catch (e) {
      error = e as string;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadProjects();
  });

  function formatDate(dateStr: string) {
    return new Date(dateStr).toLocaleDateString('pt-BR', {
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  function getFormatIcon(format: string) {
    const icons: Record<string, string> = {
      epub: '📚',
      docx: '📄',
      pdf: '📕',
      txt: '📝',
      md: '📝',
      srt: '🎬',
      vtt: '🎬',
      html: '🌐',
    };
    return icons[format.toLowerCase()] || '📄';
  }
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold text-surface-900 dark:text-surface-100">Meus Projetos</h1>
    <button
      class="btn-primary"
      onclick={() => onNavigate('detail')}
    >
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
      </svg>
      Novo Projeto
    </button>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
      <span class="ml-3 text-surface-600 dark:text-surface-400">Carregando projetos...</span>
    </div>
  {:else if error}
    <div class="card p-6 text-center">
      <p class="text-red-600 dark:text-red-400">Erro ao carregar: {error}</p>
      <button class="btn-secondary mt-4" onclick={loadProjects}>Tentar novamente</button>
    </div>
  {:else if projects.length === 0}
    <div class="card p-12 text-center">
      <svg class="mx-auto h-16 w-16 text-surface-300 dark:text-surface-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
      </svg>
      <h3 class="mt-4 text-lg font-medium text-surface-900 dark:text-surface-100">Nenhum projeto</h3>
      <p class="mt-2 text-surface-500 dark:text-surface-400">Clique em "Novo Projeto" para começar a traduzir</p>
    </div>
  {:else}
    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each projects as project}
        <article class="card p-4 hover:shadow-md transition-shadow">
          <div class="flex items-start justify-between">
            <div class="flex-1 min-w-0">
              <span class="inline-flex items-center gap-2 text-2xl">{getFormatIcon(project.source_format)}</span>
              <h3 class="mt-2 font-semibold text-surface-900 dark:text-surface-100 truncate">{project.name}</h3>
              <p class="mt-1 text-sm text-surface-500 dark:text-surface-400 truncate">{project.source_file}</p>
              <div class="mt-3 flex items-center gap-4 text-xs text-surface-500 dark:text-surface-400">
                <span class="flex items-center gap-1">
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  {formatDate(project.updated_at)}
                </span>
                <span class="flex items-center gap-1">
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                  </svg>
                  {project.source_format.toUpperCase()}
                </span>
              </div>
            </div>
            <div class="ml-4 flex-shrink-0">
              <div class="w-12 h-12 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
                <span class="text-2xl font-bold text-primary-600 dark:text-primary-400">{project.overall_progress}%</span>
              </div>
            </div>
          </div>
          <div class="mt-4 pt-4 border-t border-surface-200 dark:border-surface-700">
            <div class="w-full h-2 bg-surface-200 dark:bg-surface-700 rounded-full overflow-hidden">
              <div
                class="h-full bg-primary-600 rounded-full transition-all duration-300"
                style="width: {project.overall_progress}%"
              ></div>
            </div>
            <div class="mt-2 flex justify-between text-xs text-surface-500 dark:text-surface-400">
              <span>Progresso geral</span>
              <span>{project.overall_progress}% completo</span>
            </div>
          </div>
          <div class="mt-4 flex gap-2">
            <button
              class="btn-secondary flex-1"
              onclick={() => onNavigate('detail', project.id)}
            >
              Abrir
            </button>
            <button class="btn-ghost text-red-600 hover:text-red-700" onclick={() => deleteProject(project.id)}>
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</div>

<script lang="ts">
  async function deleteProject(id: string) {
    if (!confirm('Tem certeza que deseja excluir este projeto?')) return;
    try {
      await invoke('delete_project', { id });
      await loadProjects();
    } catch (e) {
      alert('Erro ao excluir: ' + e);
    }
  }
</script>