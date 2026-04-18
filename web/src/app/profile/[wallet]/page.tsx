'use client';

import { use } from 'react';
import { notFound } from 'next/navigation';
import { PageShell } from '@/components/layout';
import {
  ProfileHeader,
  ProfileStats,
  ProfilePositions,
  ProfileActivity,
  CopyTraderButton,
} from '@/components/profile';

const EVM_ADDRESS_REGEX = /^0x[0-9a-fA-F]{40}$/;

interface Props {
  params: Promise<{ wallet: string }>;
}

export default function ProfilePage({ params }: Props) {
  const { wallet } = use(params);

  if (!EVM_ADDRESS_REGEX.test(wallet)) {
    notFound();
  }

  return (
    <PageShell>
      <div className="py-8 space-y-8">
        <div className="flex items-start justify-between gap-4">
          <div className="flex-1">
            <ProfileHeader wallet={wallet} />
          </div>
          <div className="flex-shrink-0 pt-2">
            <CopyTraderButton wallet={wallet} />
          </div>
        </div>
        <ProfileStats wallet={wallet} />
        <ProfilePositions wallet={wallet} />
        <ProfileActivity wallet={wallet} />
      </div>
    </PageShell>
  );
}
