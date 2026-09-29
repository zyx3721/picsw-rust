<script setup lang="ts">
import { computed, ref } from 'vue';
import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-vue-next';

const props = defineProps<{ initial?: { start: string; end: string } | null }>();
const emit = defineEmits<{ apply: [{ start: string; end: string }]; clear: [] }>();

const weekdays = ['一', '二', '三', '四', '五', '六', '日'];
const anchorMonth = ref(props.initial ? monthAnchorOf(props.initial.start) : new Date());
const pendingStart = ref(props.initial?.start || '');
const pendingEnd = ref(props.initial?.end || '');
const hoverDate = ref('');

const leftMonth = computed(() => new Date(anchorMonth.value.getFullYear(), anchorMonth.value.getMonth(), 1));
const rightMonth = computed(() => new Date(anchorMonth.value.getFullYear(), anchorMonth.value.getMonth() + 1, 1));
const canApply = computed(() => Boolean(pendingStart.value && pendingEnd.value));

function monthAnchorOf(iso: string) {
  const date = new Date(`${iso}T00:00:00`);
  return Number.isNaN(date.valueOf()) ? new Date() : new Date(date.getFullYear(), date.getMonth(), 1);
}

function formatKey(date: Date) {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function monthGrid(month: Date) {
  const firstWeekday = (new Date(month.getFullYear(), month.getMonth(), 1).getDay() + 6) % 7;
  return Array.from({ length: 42 }, (_, index) => new Date(month.getFullYear(), month.getMonth(), index - firstWeekday + 1));
}

/// 两次点击确定起止：第二次点击早于起点时自动交换
function pickDate(date: Date) {
  const key = formatKey(date);
  if (pendingStart.value && !pendingEnd.value) {
    const [start, end] = pendingStart.value <= key ? [pendingStart.value, key] : [key, pendingStart.value];
    pendingStart.value = start;
    pendingEnd.value = end;
    return;
  }
  pendingStart.value = key;
  pendingEnd.value = '';
  hoverDate.value = '';
}

function dayClass(date: Date, month: Date) {
  const key = formatKey(date);
  const classes = [];
  if (date.getMonth() !== month.getMonth()) classes.push('muted');
  if (key === pendingStart.value || key === pendingEnd.value) {
    classes.push('selected');
    return classes.join(' ');
  }
  const rangeEnd = pendingEnd.value || (pendingStart.value ? hoverDate.value : '');
  if (pendingStart.value && rangeEnd) {
    const [from, to] = pendingStart.value <= rangeEnd ? [pendingStart.value, rangeEnd] : [rangeEnd, pendingStart.value];
    if (key > from && key < to) classes.push('in-range');
  }
  return classes.join(' ');
}

function shiftYear(delta: number) {
  anchorMonth.value = new Date(anchorMonth.value.getFullYear() + delta, anchorMonth.value.getMonth(), 1);
}

function shiftMonth(delta: number) {
  anchorMonth.value = new Date(anchorMonth.value.getFullYear(), anchorMonth.value.getMonth() + delta, 1);
}

function applySelection() {
  if (!canApply.value) return;
  const [start, end] = pendingStart.value <= pendingEnd.value ? [pendingStart.value, pendingEnd.value] : [pendingEnd.value, pendingStart.value];
  emit('apply', { start, end });
}

function clearSelection() {
  pendingStart.value = '';
  pendingEnd.value = '';
  hoverDate.value = '';
  emit('clear');
}
</script>

<template>
  <div class="date-range-panel" role="dialog" aria-label="选择日期范围">
    <div class="date-range-months">
      <div class="date-range-month">
        <div class="date-range-month-head">
          <div class="date-range-nav">
            <button type="button" aria-label="上一年" @click="shiftYear(-1)"><ChevronsLeft :size="14" /></button>
            <button type="button" aria-label="上一个月" @click="shiftMonth(-1)"><ChevronLeft :size="14" /></button>
          </div>
          <span class="date-range-month-title">{{ leftMonth.getFullYear() }} 年 {{ leftMonth.getMonth() + 1 }} 月</span>
          <span class="date-range-nav-spacer"></span>
        </div>
        <div class="date-range-grid">
          <span v-for="day in weekdays" :key="day" class="date-range-weekday">{{ day }}</span>
          <button
            v-for="date in monthGrid(leftMonth)"
            :key="formatKey(date)"
            type="button"
            :class="['date-range-day', dayClass(date, leftMonth)]"
            @click="pickDate(date)"
            @mouseenter="hoverDate = formatKey(date)"
          >{{ date.getDate() }}</button>
        </div>
      </div>
      <div class="date-range-months-divider"></div>
      <div class="date-range-month">
        <div class="date-range-month-head">
          <span class="date-range-nav-spacer"></span>
          <span class="date-range-month-title">{{ rightMonth.getFullYear() }} 年 {{ rightMonth.getMonth() + 1 }} 月</span>
          <div class="date-range-nav">
            <button type="button" aria-label="下一个月" @click="shiftMonth(1)"><ChevronRight :size="14" /></button>
            <button type="button" aria-label="下一年" @click="shiftYear(1)"><ChevronsRight :size="14" /></button>
          </div>
        </div>
        <div class="date-range-grid">
          <span v-for="day in weekdays" :key="day" class="date-range-weekday">{{ day }}</span>
          <button
            v-for="date in monthGrid(rightMonth)"
            :key="formatKey(date)"
            type="button"
            :class="['date-range-day', dayClass(date, rightMonth)]"
            @click="pickDate(date)"
            @mouseenter="hoverDate = formatKey(date)"
          >{{ date.getDate() }}</button>
        </div>
      </div>
    </div>
    <div class="date-range-footer">
      <button class="ghost" type="button" @click="clearSelection">清空</button>
      <button class="secondary" type="button" :disabled="!canApply" @click="applySelection">确定</button>
    </div>
  </div>
</template>
