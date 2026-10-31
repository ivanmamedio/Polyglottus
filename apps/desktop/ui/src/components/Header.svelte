<script lang="ts">
  export let onNavigate: (view: 'list' | 'detail' | 'settings', projectId?: string) => void;
  export let currentView: 'list' | 'detail' | 'settings';
  export let selectedProject: string | null;
</script>

<header class="h-16 bg-white dark:bg-surface-800 border-b border-surface-200 dark:border-surface-700 flex items-center justify-between px-6">
  <div class="flex items-center gap-8">
    <h1 class="text-xl font-bold text-primary-600 dark:text-primary-400 flex items-center gap-2">
      <svg class="w-7 h-7" fill="currentColor" viewBox="0 0 24 24">
        <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93c-3.95-.49-7-3.85-7-7.93a6.99 6.99 0 01.21-1.79L7.07 2.07A8.985 8.985 0 002 12c0 4.97 4.02 9 9 9a8.985 8.985 0 005.07-1.93l-1.9-1.9c-.13.58-.21 1.17-.21 1.79 0 4.08 3.05 7.44 7 7.93v-2.02c-1.1 0-2-.9-2-2v-1c1.1 0 2-.9 2-2v-1.93c3.95.49 7 3.85 7 7.93 0 .62-.08 1.21-.21 1.79l1.71 1.71c.37-.37.74-.74 1.11-1.11V21c-1.1 0-2-.9-2-2v-1.93c3.95.49 7 3.85 7 7.93a6.99 6.99 0 01-.21 1.79l1.9 1.9A8.985 8.985 0 0112 22c-4.97 0-9-4.03-9-9 0-.62.08-1.21.21-1.79l1.9 1.9c.13.58.21 1.17.21 1.79 0-4.08-3.05-7.44-7-7.93v2.02c1.1 0 2 .9 2 2v1c-1.1 0-2 .9-2 2v1.93c-3.95-.49-7-3.85-7-7.93a6.99 6.99 0 01-.21-1.79l-1.9-1.9A8.985 8.985 0 012 12c0-4.97 4.02-9 9-9a8.985 8.985 0 015.07 1.93l1.9-1.9c-.13-.58-.21-1.17-.21-1.79 0-4.08 3.05-7.44 7-7.93V3c1.1 0 2 .9 2 2v1c-1.1 0-2 .9-2 2v1.93c-3.95-.49-7-3.85-7-7.93a6.99 6.99 0 00-.21 1.79l-1.9 1.9c-.37.37-.74.74-1.11 1.11V3c1.1 0 2 .9 2 2v1c-1.1 0-2 .9-2 2V4.07c3.95-.49 7-3.85 7-7.93a6.99 6.99 0 00.21-1.79L16.93 2.07A8.985 8.985 0 0022 12c0 4.97-4.02 9-9 9a8.985 8.985 0 01-5.07-1.93l-1.9 1.9c.13-.58.21-1.17.21-1.79 0-4.08-3.05-7.44-7-7.93V2c-1.1 0-2-.9-2-2V1c1.1 0 2 .9 2 2v1.93c3.95.49 7 3.85 7 7.93a6.99 6.99 0 00.21 1.79l1.9 1.9A8.985 8.985 0 0022 12c0-4.97-4.02-9-9-9z"/>
      </svg>
      Polyglottus
    </h1>

    <nav class="flex items-center gap-1">
      <button
        class={['px-3 py-2 rounded-lg text-sm font-medium transition-colors', currentView === 'list' ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-300' : 'text-surface-600 hover:bg-surface-100 dark:text-surface-400 dark:hover:bg-surface-800']}
        onclick={() => onNavigate('list')}
      >
        Projetos
      </button>
      <button
        class={['px-3 py-2 rounded-lg text-sm font-medium transition-colors', currentView === 'settings' ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-300' : 'text-surface-600 hover:bg-surface-100 dark:text-surface-400 dark:hover:bg-surface-800']}
        onclick={() => onNavigate('settings')}
      >
        Modelos
      </button>
    </nav>
  </div>

  <div class="flex items-center gap-4">
    <div class="hidden sm:block text-sm text-surface-500 dark:text-surface-400">
      {#if currentView === 'detail' && selectedProject}
        Projeto selecionado
      {/if}
    </div>

    <button class="btn-ghost p-2" title="Tema">
      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
      </svg>
    </button>
  </div>
</header>