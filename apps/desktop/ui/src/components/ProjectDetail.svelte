<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';

  export let projectId: string;
  export let onBack: () => void;

  interface JobSummary {
    id: string;
    project_id: string;
    target_language: string;
    state: string;
    progress: {
      total: number;
      completed: number;
      failed: number;
      flagged: number;
      pending: number;
      percent: number;
      elapsed_secs: number;
      eta_secs: number | null;
      units_per_second: number;
    };
    created_at: string;
    updated_at: string;
  }

  interface UnitDto {
    id: string;
    source_text: string;
    translated_text: string | null;
    kind: string;
    state: string;
    section: string;
    qa_warnings: Array<{ code: string; message: string; severity: string }>;
  }

  let jobs: JobSummary[] = [];
  let selectedJob: JobSummary | null = null;
  let units: UnitDto[] = [];
  let loading = true;
  let translating = false;
  let showTranslateModal = false;
  let showExportModal = false;

  let translateConfig = {
    provider: 'ollama',
    model: '',
    url: '',
    batchSize: 10,
    maxParallel: 3,
    targetLang: 'pt-BR',
    outputFormat: 'same',
    bilingual: false,
  };

  let exportConfig = {
    format: 'same',
    bilingual: false,
  };

  let unsubscribeEvents: () => void;
  let filterState = 'all';

  $: filteredUnits = units.filter(u => filterState === 'all' || u.state === filterState);

  async function loadJobs() {
    try {
      const result = await invoke<JobSummary[]>('list_jobs', { projectId });
      jobs = result;
      if (jobs.length > 0 && !selectedJob) {
        selectedJob = jobs[0];
        await loadUnits();
      }
    } catch (e) {
      console.error('Failed to load jobs:', e);
    } finally {
      loading = false;
    }
  }

  async function loadUnits() {
    if (!selectedJob) return;
    try {
      const states = ['Pending', 'Translated', 'Flagged', 'Reviewed', 'Failed'];
      const result = await invoke<UnitDto[]>('get_units', {
        jobId: selectedJob.id,
        states,
      });
      units = result;
    } catch (e) {
      console.error('Failed to load units:', e);
    }
  }

  function formatTime(secs: number) {
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
  }

  function getStateColor(state: string) {
    switch (state) {
      case 'Running': return 'text-blue-600 bg-blue-100 dark:bg-blue-900/30';
      case 'Paused': return 'text-yellow-600 bg-yellow-100 dark:bg-yellow-900/30';
      case 'Completed': return 'text-green-600 bg-green-100 dark:bg-green-900/30';
      case 'Failed': return 'text-red-600 bg-red-100 dark:bg-red-900/30';
      case 'Cancelled': return 'text-surface-600 bg-surface-100 dark:bg-surface-800';
      default: return 'text-surface-600 bg-surface-100 dark:bg-surface-800';
    }
  }

  function getUnitStateColor(state: string) {
    switch (state) {
      case 'Pending': return 'text-surface-500';
      case 'Translated': return 'text-blue-600';
      case 'Flagged': return 'text-yellow-600';
      case 'Reviewed': return 'text-green-600';
      case 'Failed': return 'text-red-600';
      default: return 'text-surface-500';
    }
  }

  async function startTranslation() {
    if (!selectedJob) return;
    translating = true;
    showTranslateModal = false;
    try {
      await invoke('start_translation', {
        jobId: selectedJob.id,
        ...translateConfig,
      });
    } catch (e) {
      alert('Erro ao iniciar tradução: ' + e);
      translating = false;
    }
  }

  async function pauseTranslation() {
    if (!selectedJob) return;
    try {
      await invoke('pause_translation', { jobId: selectedJob.id });
    } catch (e) {
      alert('Erro ao pausar: ' + e);
    }
  }

  async function resumeTranslation() {
    if (!selectedJob) return;
    try {
      await invoke('resume_translation', { jobId: selectedJob.id });
    } catch (e) {
      alert('Erro ao retomar: ' + e);
    }
  }

  async function cancelTranslation() {
    if (!selectedJob) return;
    if (!confirm('Tem certeza que deseja cancelar? O progresso será salvo.')) return;
    try {
      await invoke('cancel_translation', { jobId: selectedJob.id });
    } catch (e) {
      alert('Erro ao cancelar: ' + e);
    }
  }

  async function exportBook() {
    if (!selectedJob) return;
    showExportModal = false;
    try {
      const outputPath = await invoke<string>('export_book', {
        jobId: selectedJob.id,
        outputPath: '', // TODO: usar dialog para escolher
        format: exportConfig.format,
        bilingual: exportConfig.bilingual,
      });
      alert('Exportado para: ' + outputPath);
    } catch (e) {
      alert('Erro ao exportar: ' + e);
    }
  }

  async function editUnit(unit: UnitDto) {
    // TODO: abrir modal de edição
    alert('Editar: ' + unit.source_text.substring(0, 50));
  }

  async function retraduzeUnit(id: string) {
    try {
      await invoke('update_unit', { unitId: id, translation: '', action: 'retranslate' });
      await loadUnits();
    } catch (e) {
      alert('Erro: ' + e);
    }
  }

  onMount(async () => {
    await loadJobs();

    // Listen for real-time events
    unsubscribeEvents = await listen('app-event', (event) => {
      const payload = event.payload;
      if (payload.type === 'JobProgress' && selectedJob?.id === payload.job_id) {
        const jobIndex = jobs.findIndex(j => j.id === payload.job_id);
        if (jobIndex >= 0) {
          jobs[jobIndex].progress = payload.progress;
          jobs = [...jobs];
        }
      }
    });
  });

  onDestroy(() => {
    if (unsubscribeEvents) unsubscribeEvents();
  });
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-4">
      <button class="btn-ghost" onclick={onBack}>
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
        </svg>
      </button>
      <div>
        <h1 class="text-2xl font-bold text-surface-900 dark:text-surface-100">Detalhes do Projeto</h1>
        <p class="text-surface-500 dark:text-surface-400">Gerencie jobs de tradução</p>
      </div>
    </div>

    <div class="flex items-center gap-2">
      <button class="btn-primary" onclick={() => showTranslateModal = true} disabled={translating || !selectedJob?.progress.pending}>
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
        </svg>
        Traduzir
      </button>
      <button class="btn-secondary" onclick={() => showExportModal = true} disabled={!selectedJob || selectedJob.state !== 'Completed'}>
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
        </svg>
        Exportar
      </button>
    </div>
  </div>

  {#if loading}
    <div class="flex items-center justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-600"></div>
    </div>
  {:else}
    <!-- Jobs List -->
    <div class="card">
      <div class="p-4 border-b border-surface-200 dark:border-surface-700">
        <h2 class="text-lg font-semibold">Jobs de Tradução</h2>
      </div>
      <div class="divide-y divide-surface-200 dark:divide-surface-700">
        {#each jobs as job}
          <div class="p-4 hover:bg-surface-50 dark:hover:bg-surface-800/50 transition-colors" class:selected={selectedJob?.id === job.id} onclick={() => { selectedJob = job; loadUnits(); }}>
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-4">
                <span class="px-3 py-1 rounded-full text-sm font-medium {getStateColor(job.state)}">{job.state}</span>
                <div>
                  <p class="font-medium text-surface-900 dark:text-surface-100">Para {job.target_language}</p>
                  <p class="text-sm text-surface-500 dark:text-surface-400">Criado em {new Date(job.created_at).toLocaleDateString('pt-BR')}</p>
                </div>
              </div>
              <div class="flex items-center gap-4">
                <div class="w-32 h-2 bg-surface-200 dark:bg-surface-700 rounded-full overflow-hidden">
                  <div class="h-full bg-primary-600 rounded-full transition-all duration-300" style="width: {job.progress.percent}%"></div>
                </div>
                <span class="text-sm font-medium text-surface-700 dark:text-surface-300 w-16 text-right">{job.progress.percent}%</span>
              </div>
            </div>
            <div class="mt-2 flex items-center gap-4 text-sm text-surface-500 dark:text-surface-400">
              <span>{job.progress.completed}/{job.progress.total} unidades</span>
              {#if job.progress.eta_secs !== null}
                <span>ETA: {formatTime(job.progress.eta_secs)}</span>
              {/if}
              <span>{job.progress.units_per_second.toFixed(1)} un/s</span>
            </div>
            {#if job.state === 'Running'}
              <div class="mt-2 flex gap-2">
                <button class="btn-ghost text-sm" onclick={pauseTranslation}>Pausar</button>
                <button class="btn-ghost text-sm text-red-600" onclick={cancelTranslation}>Cancelar</button>
              </div>
            {:else if job.state === 'Paused'}
              <div class="mt-2 flex gap-2">
                <button class="btn-primary text-sm" onclick={resumeTranslation}>Retomar</button>
                <button class="btn-ghost text-sm text-red-600" onclick={cancelTranslation}>Cancelar</button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    {#if selectedJob}
      <!-- Units Table -->
      <div class="card">
        <div class="p-4 border-b border-surface-200 dark:border-surface-700 flex items-center justify-between">
          <h2 class="text-lg font-semibold">Unidades ({units.length})</h2>
          <div class="flex gap-2">
            <select class="input w-auto" bind:value={filterState} on:change={loadUnits}>
              <option value="all">Todas</option>
              <option value="Pending">Pendentes</option>
              <option value="Translated">Traduzidas</option>
              <option value="Flagged">Com aviso</option>
              <option value="Reviewed">Revisadas</option>
              <option value="Failed">Falhas</option>
            </select>
          </div>
        </div>
        <div class="overflow-x-auto">
          <table class="w-full">
            <thead>
              <tr class="text-left text-sm text-surface-500 dark:text-surface-400 border-b border-surface-200 dark:border-surface-700">
                <th class="pb-2 pr-4 w-8"></th>
                <th class="pb-2 pr-4">Tipo</th>
                <th class="pb-2 pr-4">Original</th>
                <th class="pb-2 pr-4">Tradução</th>
                <th class="pb-2 pr-4">Estado</th>
                <th class="pb-2 pr-4">QA</th>
                <th class="pb-2 pr-4 w-24">Ações</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-surface-200 dark:divide-surface-700">
              {#each filteredUnits as unit}
                <tr class="hover:bg-surface-50 dark:hover:bg-surface-800/50">
                  <td class="py-3 pr-4">
                    <input type="checkbox" disabled checked={unit.state !== 'Pending'} />
                  </td>
                  <td class="py-3 pr-4 text-sm text-surface-600 dark:text-surface-400">{unit.kind}</td>
                  <td class="py-3 pr-4 max-w-xs truncate text-surface-900 dark:text-surface-100" title={unit.source_text}>{unit.source_text}</td>
                  <td class="py-3 pr-4 max-w-xs truncate text-surface-700 dark:text-surface-300" title={unit.translated_text || ''}>{unit.translated_text || '—'}</td>
                  <td class="py-3 pr-4">
                    <span class="px-2 py-0.5 rounded text-xs font-medium" style="color: {getUnitStateColor(unit.state)}; background: {getUnitStateColor(unit.state)}20;">
                      {unit.state}
                    </span>
                  </td>
                  <td class="py-3 pr-4">
                    {#if unit.qa_warnings.length > 0}
                      <span class="badge-warning">{unit.qa_warnings.length} aviso(s)</span>
                    {:else}
                      <span class="text-surface-400 dark:text-surface-500">—</span>
                    {/if}
                  </td>
                  <td class="py-3 pr-4">
                    <div class="flex items-center gap-1">
                      {#if unit.state === 'Flagged' || unit.state === 'Failed'}
                        <button class="btn-ghost p-1.5" title="Editar/Revisar" onclick={() => editUnit(unit)}>
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" /></svg>
                        </button>
                        <button class="btn-ghost p-1.5 text-blue-600" title="Retraduzir" onclick={() => retraduzeUnit(unit.id)}>
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
                        </button>
                      {/if}
                    </div>
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