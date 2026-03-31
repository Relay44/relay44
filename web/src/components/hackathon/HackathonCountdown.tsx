'use client';

import { useState, useEffect } from 'react';

interface HackathonCountdownProps {
  targetTime: string;
  label: string;
}

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

export function HackathonCountdown({ targetTime, label }: HackathonCountdownProps) {
  const [remaining, setRemaining] = useState('');

  useEffect(() => {
    function update() {
      const diff = new Date(targetTime).getTime() - Date.now();
      if (diff <= 0) {
        setRemaining('');
        return;
      }
      const d = Math.floor(diff / 86400000);
      const h = Math.floor((diff % 86400000) / 3600000);
      const m = Math.floor((diff % 3600000) / 60000);
      const s = Math.floor((diff % 60000) / 1000);
      setRemaining(d > 0 ? `${d}d ${pad(h)}:${pad(m)}:${pad(s)}` : `${pad(h)}:${pad(m)}:${pad(s)}`);
    }

    update();
    const interval = setInterval(update, 1000);
    return () => clearInterval(interval);
  }, [targetTime]);

  if (!remaining) return null;

  return (
    <div className="flex items-center gap-2 text-sm text-text-secondary">
      <span>{label}</span>
      <span className="font-mono font-medium text-text-primary">{remaining}</span>
    </div>
  );
}
