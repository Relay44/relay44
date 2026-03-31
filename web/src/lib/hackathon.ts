export const STATUS_VARIANT: Record<string, 'bid' | 'accent' | 'default' | 'ask'> = {
  upcoming: 'accent',
  active: 'bid',
  completed: 'default',
  cancelled: 'ask',
};

export const HACKATHON_STATUSES = ['upcoming', 'active', 'completed', 'cancelled'] as const;
