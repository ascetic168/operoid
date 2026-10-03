<script setup lang="ts">
import { ForbiddenView } from '@front/ui'
import { useI18n } from 'vue-i18n'
const { t } = useI18n()
</script>

<template>
  <ForbiddenView :title="t('forbidden.title')" :message="t('forbidden.message')" :home="t('forbidden.home')" />
</template>
