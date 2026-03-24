import { SecurityAuditPageClient } from '@/components/admin/SecurityAuditPageClient';
import { buildPageMetadata } from '@/lib/seo';

export const metadata = buildPageMetadata({
  title: 'Security audit',
  description: 'Internal security audit preparation for relay44 administrators.',
  path: '/admin/security',
  noIndex: true,
});

export default function SecurityAuditPage() {
  return <SecurityAuditPageClient />;
}
