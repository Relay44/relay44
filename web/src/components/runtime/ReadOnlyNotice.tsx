import Link from 'next/link';
import { Card } from '@/components/ui';
import { cn } from '@/lib/utils';

interface ReadOnlyNoticeProps {
  title?: string;
  body?: string;
  actionHref?: string;
  actionLabel?: string;
  className?: string;
}

const DEFAULT_BODY =
  'This action is not available in the current environment.';

export function ReadOnlyNotice({
  title = 'Currently unavailable',
  body = DEFAULT_BODY,
  actionHref,
  actionLabel,
  className,
}: ReadOnlyNoticeProps) {
  return (
    <Card className={cn('border-accent/30 bg-accent/5', className)}>
      <div className="space-y-3">
        <div>
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-accent">
            Availability
          </p>
          <h2 className="mt-2 text-lg font-semibold text-text-primary">{title}</h2>
        </div>
        <p className="text-sm text-text-secondary">{body}</p>
        {actionHref && actionLabel ? (
          <Link
            href={actionHref}
            className="inline-flex h-9 items-center border border-border px-3 text-sm text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
          >
            {actionLabel}
          </Link>
        ) : null}
      </div>
    </Card>
  );
}
