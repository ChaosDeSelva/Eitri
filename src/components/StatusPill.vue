<script setup lang="ts">
import Pill from './Pill.vue'
import type { ItemStatus } from '@/lib/api'
import { STATUS_LABEL, STATUS_TONE } from '@/lib/format'

defineProps<{ status: ItemStatus }>()
</script>

<template>
  <Pill :tone="STATUS_TONE[status]" dot :pulse="status === 'drawing'">{{ STATUS_LABEL[status] }}</Pill>
</template>
