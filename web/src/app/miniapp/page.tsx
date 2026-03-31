'use client';

import dynamic from 'next/dynamic';

const MiniAppHome = dynamic(() => import('./MiniAppHome'), {
  ssr: false,
  loading: () => (
    <div className="flex justify-center py-8">
      <div className="h-5 w-5 border-2 border-accent border-t-transparent rounded-full animate-spin" />
    </div>
  ),
});

export default function MiniAppPage() {
  return <MiniAppHome />;
}
