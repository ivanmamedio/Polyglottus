<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import ProjectList from './components/ProjectList.svelte';
  import ProjectDetail from './components/ProjectDetail.svelte';
  import Settings from './components/Settings.svelte';
  import Header from './components/Header.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import Toast from './components/Toast.svelte';

  type View = 'list' | 'detail' | 'settings';

  let currentView: View = 'list';
  let selectedProject: string | null = null;
  let toasts: Array<{ id: number; message: string; type: 'success' | 'error' | 'info' }> = [];
  let toastId = 0;

  function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
    const id = ++toastId;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => {
      toasts = toasts.filter(t => t.id !== id);
    }, 5000);
  }

  onMount(() => {
    // Listen for app events from backend
    listen('app-event', (event) => {
      console.log('App event:', event.payload);
      // Handle specific events
      if (event.payload.type === 'JobCompleted') {
        showToast('Tradução concluída!', 'success');
      } else if (event.payload.type === 'JobFailed') {
        showToast(`Erro: ${event.payload.error}`, 'error');
      }
    });

    listen('model-download-progress', (event) => {
      console.log('Model download progress:', event.payload);
    });
  });

  function navigateTo(view: View, projectId?: string) {
    currentView = view;
    if (projectId) selectedProject = projectId;
  }
</script>

<div class="min-h-screen flex flex-col">
  <Header onNavigate={navigateTo} currentView={currentView} selectedProject={selectedProject} />

  <div class="flex-1 flex overflow-hidden">
    <Sidebar currentView={currentView} onNavigate={navigateTo} />

    <main class="flex-1 overflow-auto p-6">
      {#if currentView === 'list'}
        <ProjectList onSelect={navigateTo} />
      {:else if currentView === 'detail' && selectedProject}
        <ProjectDetail projectId={selectedProject} onBack={() => navigateTo('list')} />
      {:else if currentView === 'settings'}
        <Settings />
      {/if}
    </main>
  </div>

  {#each toasts as toast}
    <Toast {toast} onDismiss={() => toasts = toasts.filter(t => t.id !== toast.id)} />
  {/each}
</div>