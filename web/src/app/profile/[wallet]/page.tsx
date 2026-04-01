'use client';

import { use } from 'react';
import { notFound } from 'next/navigation';
import { PageShell } from '@/components/layout';
import {
  ProfileHeader,
  ProfileStats,
  ProfilePositions,
  ProfileActivity,
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
      <div className="container mx-auto max-w-6xl px-4 py-8 space-y-8">
        <ProfileHeader wallet={wallet} />
        <ProfileStats wallet={wallet} />
        <ProfilePositions wallet={wallet} />
        <ProfileActivity wallet={wallet} />
      </div>
    </PageShell>
  );
}
