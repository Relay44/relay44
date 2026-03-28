import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Settings',
  description: 'Manage wallet connections, network preferences, and venue credentials on Relay44.',
  path: '/settings',
  noIndex: true,
});

export default function SettingsLayout({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
