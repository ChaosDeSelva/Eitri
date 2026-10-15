<script setup lang="ts">
import type { Tone } from '@/lib/format'

withDefaults(defineProps<{ tone?: Tone; dot?: boolean; pulse?: boolean }>(), { tone: 'neutral' })
</script>

<template>
  <span
    class="inline-flex h-5 items-center gap-1.5 rounded-full px-2 text-[11px] font-semibold whitespace-nowrap"
    :class="{
      'bg-surface-3 text-muted': tone === 'neutral',
      'bg-info-soft text-info': tone === 'info',
      'bg-ok-soft text-ok': tone === 'ok',
      'bg-warn-soft text-warn': tone === 'warn',
      'bg-bad-soft text-bad': tone === 'bad',
      'bg-accent-soft text-accent': tone === 'accent',
    }"
  >
    <span v-if="dot" class="relative flex size-1.5">
      <span v-if="pulse" class="absolute inline-flex size-full animate-ping rounded-full bg-current opacity-60" />
      <span class="relative inline-flex size-1.5 rounded-full bg-current" />
    </span>
    <slot />
  </span>
</template>
