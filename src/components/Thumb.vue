<script setup lang="ts">
import { computed } from 'vue'
import Icon from './Icon.vue'
import { fileUrl, type ItemStatus } from '@/lib/api'

const props = defineProps<{ file: string | null; status?: ItemStatus; alt?: string }>()
const src = computed(() => fileUrl(props.file))
</script>

<template>
  <div class="checker relative aspect-square w-full overflow-hidden rounded-md">
    <img v-if="src" :src="src" :alt="alt ?? ''" loading="lazy" decoding="async" class="size-full object-contain" draggable="false" />
    <div v-else class="flex size-full items-center justify-center text-faint">
      <div v-if="status === 'drawing'" class="flex flex-col items-center gap-2 text-info">
        <span class="size-6 animate-spin rounded-full border-2 border-current border-t-transparent" />
        <span class="text-[11px] font-semibold">Drawing</span>
      </div>
      <Icon v-else :name="status === 'failed' || status === 'refused' ? 'alert' : 'image'" :size="22" />
    </div>
    <div v-if="src && status === 'drawing'" class="absolute inset-0 flex items-center justify-center bg-black/30">
      <span class="size-6 animate-spin rounded-full border-2 border-white border-t-transparent" />
    </div>
  </div>
</template>
