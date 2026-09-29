<script setup lang="ts">
import { computed, ref } from 'vue';

type TooltipPlacement = 'top' | 'bottom';

const props = defineProps<{ label?: string; wrap?: boolean; center?: boolean }>();
const triggerRef = ref<HTMLElement | null>(null);
const tooltip = ref({ open: false, top: 0, left: 0, placement: 'top' as TooltipPlacement });

const bubbleStyle = computed(() => ({
  left: `${tooltip.value.left}px`,
  top: `${tooltip.value.top}px`,
  transform:
    tooltip.value.placement === 'top'
      ? props.center
        ? 'translate(-50%, -100%)'
        : 'translate(0, -100%)'
      : props.center
        ? 'translate(-50%, 0)'
        : 'translate(0, 0)',
}));

function showTooltip() {
  if (!props.label || !triggerRef.value) return;
  const rect = triggerRef.value.getBoundingClientRect();
  const gap = 8;
  const placement: TooltipPlacement =
    rect.top < 80 && window.innerHeight - rect.bottom > rect.top ? 'bottom' : 'top';
  // center 模式以触发元素水平中心定位（配合 translate(-50%) 居中对称）
  const left = props.center
    ? Math.min(window.innerWidth - 24, Math.max(24, rect.left + rect.width / 2))
    : Math.min(window.innerWidth - 80, Math.max(72, rect.left));
  tooltip.value = {
    open: true,
    top: placement === 'top' ? rect.top - gap : rect.bottom + gap,
    left,
    placement,
  };
}

function hideTooltip() {
  tooltip.value = { ...tooltip.value, open: false };
}
</script>

<template>
  <span ref="triggerRef" class="app-tooltip-trigger" @mouseenter="showTooltip" @mouseleave="hideTooltip">
    <slot />
    <Teleport to="body">
      <span
        v-if="tooltip.open"
        class="app-tooltip-bubble"
        :class="{ wrap: props.wrap, center: props.center }"
        :style="bubbleStyle"
        >{{ label }}</span
      >
    </Teleport>
  </span>
</template>
