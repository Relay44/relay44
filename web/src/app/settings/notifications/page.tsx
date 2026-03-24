import { PageShell } from '@/components/layout';
import { NotificationSettings } from '@/components/notifications';
import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Notification settings',
  description: 'Manage notification preferences for relay44.',
  path: '/settings/notifications',
  noIndex: true,
});

export default function NotificationSettingsPage() {
  return (
    <PageShell>
      <div className="mx-auto max-w-2xl py-2 sm:py-4">
        <NotificationSettings />
      </div>
    </PageShell>
  );
}
