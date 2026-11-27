<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  onMount(async () => {
    await invoke<any>('get_settings');
  });
</script>

<div class="space-y-6">
  <h1 class="text-xl font-bold text-gray-900">Settings</h1>
</div>
