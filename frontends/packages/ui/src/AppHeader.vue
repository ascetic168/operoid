<script setup lang="ts">
defineProps<{
  title: string
  labels: { logout: string; loggedAs: string }
  displayName: string
  roles: string[]
}>()
const emit = defineEmits<{ logout: [] }>()
</script>

<template>
  <header class="hdr">
    <strong>{{ title }}</strong>
    <span class="who">
      {{ labels.loggedAs }} <b>{{ displayName }}</b>
      <small class="roles">[{{ roles.join(', ') }}]</small>
    </span>
    <button class="out" data-test="logout" @click="emit('logout')">{{ labels.logout }}</button>
  </header>
</template>

<style scoped>
.hdr {
  display: flex; align-items: center; gap: 1rem;
  background: var(--surface); border-bottom: 1px solid var(--border);
  padding: 0.7rem 1.25rem;
}
.who { margin-left: auto; font-size: 0.88rem; color: var(--muted); }
.roles { margin-left: 0.35rem; }
.out {
  border: 1px solid var(--border); background: var(--surface); color: var(--text);
  border-radius: 0.5rem; padding: 0.4rem 0.9rem; cursor: pointer; font-size: 0.88rem;
}
.out:hover { border-color: var(--danger); color: var(--danger); }
</style>
