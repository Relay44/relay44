'use client';

import { use } from 'react';
import { notFound } from 'next/navigation';

interface Props {
  params: Promise<{ wallet: string }>;
}

export default function ProfilePage({ params }: Props) {
  use(params);
  notFound();
}
