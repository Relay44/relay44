import { ReactNode } from 'react';
import { PageShell } from '@/components/layout';
import { DocsLayout } from '@/components/docs';

export default function DocsRootLayout({ children }: { children: ReactNode }) {
  return (
    <PageShell>
      <div className="py-2 sm:py-4">
        <DocsLayout>{children}</DocsLayout>
      </div>
    </PageShell>
  );
}
