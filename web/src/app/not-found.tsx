import Link from 'next/link';
import { Button } from '@/components/ui/Button';
import { PageShell } from '@/components/layout/PageShell';

export default function NotFound() {
  return (
    <PageShell>
      <div className="flex items-center justify-center min-h-[calc(100vh-200px)]">
        <div className="flex flex-col items-center justify-center gap-8 max-w-2xl">
          {/* Large 404 in monospace */}
          <div className="text-center">
            <div className="font-mono text-8xl md:text-9xl font-bold text-text-tertiary tracking-tighter">
              404
            </div>
          </div>

          {/* Page not found heading */}
          <div className="text-center">
            <h1 className="text-2xl md:text-3xl font-bold text-text-primary mb-3">
              Page not found
            </h1>
            <p className="text-text-secondary max-w-sm">
              The page you're looking for doesn't exist or may have been moved.
            </p>
          </div>

          {/* Navigation buttons */}
          <div className="flex flex-col sm:flex-row gap-4 justify-center">
            <Link href="/markets">
              <Button variant="primary">Back to markets</Button>
            </Link>
            <Link href="/">
              <Button variant="secondary">Go home</Button>
            </Link>
          </div>

          {/* Decorative border */}
          <div className="w-full h-px bg-border mt-4" />
        </div>
      </div>
    </PageShell>
  );
}
