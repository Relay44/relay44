'use client';

import Link from 'next/link';
import { useState, useEffect } from 'react';
import { useAuth, useRuntimeMode, useSessionState } from '@/hooks';
import { api } from '@/lib/api';
import { ReadOnlyNotice } from '@/components/runtime/ReadOnlyNotice';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import type { NotificationPreferences } from '@/types';
import { cn } from '@/lib/utils';

interface ToggleProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

function Toggle({ label, description, checked, onChange, disabled }: ToggleProps) {
  return (
    <div className="flex flex-col gap-3 py-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <p className="font-medium text-text-primary">{label}</p>
        <p className="text-sm text-text-secondary">{description}</p>
      </div>
      <button
        type="button"
        onClick={() => onChange(!checked)}
        disabled={disabled}
        role="switch"
        aria-checked={checked}
        className={cn(
          'relative w-11 h-6  transition-colors cursor-pointer',
          'focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-2 focus:ring-offset-bg-primary',
          checked ? 'bg-accent' : 'bg-bg-tertiary',
          disabled && 'opacity-50 cursor-not-allowed',
          'self-start sm:self-auto'
        )}
      >
        <span
          className={cn(
            'absolute top-0.5 left-0.5 w-5 h-5  bg-white transition-transform',
            checked && 'translate-x-5'
          )}
        />
      </button>
    </div>
  );
}

export function NotificationSettings() {
  const { readOnly } = useRuntimeMode();
  const { walletConnected, login, isLoading: authLoading, error: authError } = useAuth();
  const { hasSession, sessionRestored } = useSessionState();
  const [preferences, setPreferences] = useState<NotificationPreferences>({
    orderFills: true,
    marketResolutions: true,
    priceAlerts: true,
    systemAnnouncements: true,
    decisionAlerts: true,
    emailNotifications: false,
    pushNotifications: false,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<'idle' | 'saved' | 'error'>('idle');

  useEffect(() => {
    if (!sessionRestored || !hasSession) {
      setLoading(false);
      return undefined;
    }

    let cancelled = false;

    api.getNotificationPreferences()
      .then((prefs) => {
        if (!cancelled) setPreferences(prefs);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [hasSession, sessionRestored]);

  const handleChange = (key: keyof NotificationPreferences, value: boolean) => {
    setPreferences((prev) => ({ ...prev, [key]: value }));
    setStatus('idle');
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await api.updateNotificationPreferences(preferences);
      setStatus('saved');
      setTimeout(() => setStatus('idle'), 3000);
    } catch {
      setStatus('error');
    } finally {
      setSaving(false);
    }
  };

  if (!walletConnected) {
    return (
      <Card>
        <CardContent className="space-y-4 pt-6">
          <h2 className="text-xl font-semibold text-text-primary">Connect your wallet</h2>
          <p className="text-sm text-text-secondary">
            Notification preferences are private to your wallet account.
          </p>
          <Link
            href="/settings"
            className="inline-flex h-10 items-center border border-border px-4 text-sm uppercase tracking-[0.12em] text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary"
          >
            Back to settings
          </Link>
        </CardContent>
      </Card>
    );
  }

  if (!sessionRestored) {
    return (
      <Card>
        <CardContent className="pt-6 text-sm text-text-secondary">
          Restoring wallet session...
        </CardContent>
      </Card>
    );
  }

  if (!hasSession) {
    return (
      <Card>
        <CardContent className="space-y-4 pt-6">
          <h2 className="text-xl font-semibold text-text-primary">Authenticate wallet</h2>
          <p className="text-sm text-text-secondary">
            Sign a SIWE message before editing notification preferences.
          </p>
          {authError ? <p className="text-sm text-ask">{authError}</p> : null}
          <Button type="button" onClick={() => void login()} loading={authLoading}>
            Authenticate wallet
          </Button>
        </CardContent>
      </Card>
    );
  }

  if (loading) {
    return (
      <Card>
        <CardContent className="flex items-center justify-center h-48">
          <div className="animate-pulse text-text-secondary">Loading...</div>
        </CardContent>
      </Card>
    );
  }

  if (readOnly) {
    return (
      <ReadOnlyNotice
        title="Notification settings are locked"
        body="Preference writes are disabled in read-only mode."
      />
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Notification Preferences</CardTitle>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* In-app notifications */}
