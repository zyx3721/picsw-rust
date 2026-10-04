<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';

type TooltipPlacement = 'top' | 'bottom';

const props = defineProps<{ label?: string; wrap?: boolean; center?: boolean; pre?: boolean }>();
const triggerRef = ref<HTMLElement | null>(null);
const bubbleRef = ref<HTMLElement | null>(null);
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

async function showTooltip() {
  if (!props.label || !triggerRef.value) return;
  const rect = triggerRef.value.getBoundingClientRect();
  const gap = 8;
  const left = props.center
    ? Math.min(window.innerWidth - 24, Math.max(24, rect.left + rect.width / 2))
    : Math.min(window.innerWidth - 80, Math.max(72, rect.left));
  // 先按上方渲染：气泡高度随内容变化，挂载后实测再做翻转与视口钳制
  tooltip.value = { open: true, top: rect.top - gap, left, placement: 'top' };
  await nextTick();
  const bubble = bubbleRef.value;
  if (!bubble) return;
  const height = bubble.offsetHeight;
  const fitsAbove = rect.top - gap - height >= 8;
  const fitsBelow = rect.bottom + gap + height <= window.innerHeight - 8;
  let placement: TooltipPlacement = 'top';
  let top = rect.top - gap;
  if (!fitsAbove && (fitsBelow || window.innerHeight - rect.bottom > rect.top)) {
    placement = 'bottom';
    top = rect.bottom + gap;
    if (!fitsBelow) top = Math.max(8, Math.min(top, window.innerHeight - 8 - height));
  } else if (!fitsAbove) {
    top = Math.max(Math.min(top, window.innerHeight - 8), Math.min(8 + height, window.innerHeight - 8));
  }
  tooltip.value = { open: true, top, left, placement };
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
        ref="bubbleRef"
        class="app-tooltip-bubble"
        :class="{ wrap: props.wrap, center: props.center, pre: props.pre }"
        :style="bubbleStyle"
        >{{ label }}</span
      >
    </Teleport>
  </span>
</template>
