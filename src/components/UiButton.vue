<script setup lang="ts">
import Icon from './Icon.vue'

withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: 'sm' | 'md'
    icon?: string
    loading?: boolean
    disabled?: boolean
    type?: 'button' | 'submit'
  }>(),
  { variant: 'secondary', size: 'md', type: 'button' },
)
</script>

<template>
  <button
    :type="type"
    :disabled="disabled || loading"
    class="inline-flex items-center justify-center gap-1.5 rounded-md border font-semibold whitespace-nowrap transition-[background,border-color,box-shadow,transform] active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50"
    :class="[
      size === 'sm' ? 'h-7 px-2.5 text-xs' : 'h-8 px-3 text-[13px]',
      variant === 'primary' && 'border-transparent bg-accent text-accent-fg shadow-[0_2px_0_0_rgba(0,0,0,0.18)] hover:brightness-105',
      variant === 'secondary' && 'border-line bg-surface text-fg shadow-[0_1px_0_0_var(--border)] hover:border-line-strong',
      variant === 'ghost' && 'border-transparent bg-transparent text-muted hover:bg-surface-3 hover:text-fg',
      variant === 'danger' && 'border-transparent bg-bad-soft text-bad hover:brightness-95',
    ]"
  >
    <span v-if="loading" class="size-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" />
    <Icon v-else-if="icon" :name="icon" :size="size === 'sm' ? 13 : 15" />
    <slot />
  </button>
</template>
