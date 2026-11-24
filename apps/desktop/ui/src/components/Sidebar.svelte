<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let currentView: string;
  export let onNavigate: (view: string, projectId?: string) => void;

  const dispatch = createEventDispatcher();
</script>

<aside class="w-64 bg-gray-50 border-r border-gray-200 flex-shrink-0">
  <div class="p-4">
    <h2 class="text-sm font-semibold text-gray-700 uppercase tracking-wider mb-6">Projects</h2>
    <nav>
      <button
        onclick={() => onNavigate('list')}
        class={currentView === 'list'
          ? 'w-full flex items-center gap-3 px-3 py-2 bg-indigo-600 text-white rounded-md'
          : 'w-full flex items-center gap-3 px-3 py-2 text-gray-600 hover:bg-gray-100 hover:text-gray-900 rounded-md transition'}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
        </svg>
        All Projects
      </button>
    </nav>
  </div>
</aside>
