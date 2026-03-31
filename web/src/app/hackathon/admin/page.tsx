'use client';

import { useState, useMemo } from 'react';
import { PageShell } from '@/components/layout';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useHackathons } from '@/hooks/useHackathons';
import { isAdminWallet } from '@/lib/admin';
import { useToast } from '@/components/ui/Toast';
import { api } from '@/lib/api';
import type { Hackathon } from '@/types';

export default function HackathonAdminPage() {
  const { address } = useBaseWallet();
  const isAdmin = useMemo(() => isAdminWallet(address), [address]);
  const { data, refetch } = useHackathons();
  const { addToast } = useToast();

  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState({
    name: '',
    description: '',
    prizePoolUsdc: '1000',
    startTime: '',
    endTime: '',
  });

  if (!address) {
    return (
      <PageShell>
        <div className="container mx-auto max-w-4xl px-4 py-8">
          <Card>
            <CardContent className="py-8 text-center text-text-secondary">
              Connect your wallet to access admin panel.
            </CardContent>
          </Card>
        </div>
      </PageShell>
    );
  }

  if (!isAdmin) {
    return (
      <PageShell>
        <div className="container mx-auto max-w-4xl px-4 py-8">
          <Card>
            <CardContent className="py-8 text-center text-text-secondary">
              Access denied. Admin wallet required.
            </CardContent>
          </Card>
        </div>
      </PageShell>
    );
  }

  const handleCreate = async () => {
    if (!form.name || !form.startTime || !form.endTime) {
      addToast('Fill in all required fields', 'error');
      return;
    }
    try {
      setCreating(true);
      await api.createHackathon({
        name: form.name,
        description: form.description,
        prizePoolUsdc: parseFloat(form.prizePoolUsdc) || 0,
        startTime: new Date(form.startTime).toISOString(),
        endTime: new Date(form.endTime).toISOString(),
      });
      addToast('Hackathon created', 'success');
      setForm({ name: '', description: '', prizePoolUsdc: '1000', startTime: '', endTime: '' });
      refetch();
    } catch (err: unknown) {
      addToast((err as Error).message || 'Failed to create', 'error');
    } finally {
      setCreating(false);
    }
  };

  const handleStatusChange = async (id: string, status: string) => {
    try {
      await api.updateHackathon(id, { status });
      addToast(`Status updated to ${status}`, 'success');
      refetch();
    } catch (err: unknown) {
      addToast((err as Error).message || 'Failed to update', 'error');
    }
  };

  const handleSnapshot = async (id: string) => {
    try {
      const result = await api.triggerHackathonSnapshot(id);
      addToast(`Snapshot taken: ${result.snapshotCount} entries`, 'success');
    } catch (err: unknown) {
      addToast((err as Error).message || 'Snapshot failed', 'error');
    }
  };

  const hackathons = data?.hackathons || [];

  return (
    <PageShell>
      <div className="container mx-auto max-w-4xl px-4 py-8 space-y-6">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">Hackathon Admin</h1>
          <Badge variant="accent">Admin Mode</Badge>
        </div>

        {/* Create form */}
        <Card>
          <CardHeader>
            <CardTitle>Create Hackathon</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <input
              type="text"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="Hackathon name"
              className="w-full px-3 py-2 text-sm bg-bg-secondary border border-border focus:border-accent focus:outline-none"
            />
            <textarea
              value={form.description}
              onChange={(e) => setForm({ ...form, description: e.target.value })}
              placeholder="Description"
              rows={3}
              className="w-full px-3 py-2 text-sm bg-bg-secondary border border-border focus:border-accent focus:outline-none resize-none"
            />
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div>
                <label className="text-xs text-text-muted block mb-1">Prize (USDC)</label>
                <input
                  type="number"
                  value={form.prizePoolUsdc}
                  onChange={(e) => setForm({ ...form, prizePoolUsdc: e.target.value })}
                  className="w-full px-3 py-2 text-sm bg-bg-secondary border border-border focus:border-accent focus:outline-none"
                />
              </div>
              <div>
                <label className="text-xs text-text-muted block mb-1">Start</label>
                <input
                  type="datetime-local"
                  value={form.startTime}
                  onChange={(e) => setForm({ ...form, startTime: e.target.value })}
                  className="w-full px-3 py-2 text-sm bg-bg-secondary border border-border focus:border-accent focus:outline-none"
                />
              </div>
              <div>
                <label className="text-xs text-text-muted block mb-1">End</label>
                <input
                  type="datetime-local"
                  value={form.endTime}
                  onChange={(e) => setForm({ ...form, endTime: e.target.value })}
                  className="w-full px-3 py-2 text-sm bg-bg-secondary border border-border focus:border-accent focus:outline-none"
                />
              </div>
            </div>
            <Button onClick={handleCreate} disabled={creating}>
              {creating ? 'Creating...' : 'Create Hackathon'}
            </Button>
          </CardContent>
        </Card>

        {/* Existing hackathons */}
        <Card>
          <CardHeader>
            <CardTitle>Manage Hackathons</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {hackathons.length === 0 ? (
              <p className="text-sm text-text-muted">No hackathons yet.</p>
            ) : (
              hackathons.map((h: Hackathon) => (
                <div
                  key={h.id}
                  className="p-4 border border-border space-y-2"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{h.name}</span>
                      <Badge variant={h.status === 'active' ? 'bid' : 'default'}>
                        {h.status}
                      </Badge>
                    </div>
                    <span className="text-sm text-text-muted">
                      {h.participantCount} participants
                    </span>
                  </div>

                  <div className="flex flex-wrap gap-2">
                    {h.status === 'upcoming' && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleStatusChange(h.id, 'active')}
                      >
                        Activate
                      </Button>
                    )}
                    {h.status === 'active' && (
                      <>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleStatusChange(h.id, 'completed')}
                        >
                          Complete
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleSnapshot(h.id)}
                        >
                          Take Snapshot
                        </Button>
                      </>
                    )}
                    {h.status !== 'cancelled' && h.status !== 'completed' && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleStatusChange(h.id, 'cancelled')}
                      >
                        Cancel
                      </Button>
                    )}
                  </div>
                </div>
              ))
            )}
          </CardContent>
        </Card>
      </div>
    </PageShell>
  );
}
