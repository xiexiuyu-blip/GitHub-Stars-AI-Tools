import type { ReadingStatus } from '@/types';

export type ReadingStatusOption = {
  value: ReadingStatus;
  label: string;
  shortLabel: string;
};

export const readingStatusOptions: ReadingStatusOption[] = [
  { value: 'unread', label: '未判断', shortLabel: '未判断' },
  { value: 'want_to_try', label: '想试', shortLabel: '想试' },
  { value: 'tried', label: '已试', shortLabel: '已试' },
  { value: 'in_use', label: '在用', shortLabel: '在用' },
  { value: 'watching', label: '观察中', shortLabel: '观察' },
  { value: 'deprecated', label: '弃用', shortLabel: '弃用' },
  { value: 'later', label: '稍后处理', shortLabel: '稍后' },
  { value: 'read', label: '已了解', shortLabel: '已了解' },
];

export const readingStatusLabels = Object.fromEntries(
  readingStatusOptions.map((option) => [option.value, option.label]),
) as Record<ReadingStatus, string>;

export const readingStatusShortLabels = Object.fromEntries(
  readingStatusOptions.map((option) => [option.value, option.shortLabel]),
) as Record<ReadingStatus, string>;

export function getReadingStatusLabel(status: ReadingStatus | string | null | undefined) {
  return status && status in readingStatusLabels
    ? readingStatusLabels[status as ReadingStatus]
    : readingStatusLabels.unread;
}

export function getReadingStatusShortLabel(status: ReadingStatus | string | null | undefined) {
  return status && status in readingStatusShortLabels
    ? readingStatusShortLabels[status as ReadingStatus]
    : readingStatusShortLabels.unread;
}

export function getReadingStatusToneClass(status: ReadingStatus | string | null | undefined) {
  switch (status) {
    case 'want_to_try':
      return 'border-primary/20 bg-primary/10 text-primary';
    case 'tried':
      return 'border-warning/20 bg-warning/10 text-warning';
    case 'in_use':
      return 'border-success/20 bg-success/10 text-success';
    case 'watching':
      return 'border-primary/15 bg-primary/5 text-primary';
    case 'deprecated':
      return 'border-error/20 bg-error/10 text-error';
    case 'read':
      return 'border-outline-variant/25 bg-surface-container-low text-on-surface';
    case 'later':
      return 'border-outline-variant/25 bg-surface-container-low text-on-surface-variant';
    case 'unread':
    default:
      return 'border-outline-variant/25 bg-surface-container-low text-on-surface-variant';
  }
}
