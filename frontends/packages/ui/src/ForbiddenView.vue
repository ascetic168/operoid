<script setup lang="ts">
defineProps<{ title: string; message: string; home: string }>()
</script>

<template>
  <main class="wrap">
    <div class="card center">
      <h1>403</h1>
      <p class="t">{{ title }}</p>
      <p class="muted">{{ message }}</p>
      <router-link class="home" to="/">{{ home }}</router-link>
    </div>
  </main>
</template>

<style scoped>
.wrap { min-height: 100vh; display: flex; align-items: center; justify-content: center; }
.center { text-align: center; padding: 3rem 4rem; }
h1 { margin: 0; color: var(--danger); font-size: 3rem; }
.t { font-weight: 600; margin: 0.5rem 0 0.25rem; }
.home { color: var(--accent); }
</style>
