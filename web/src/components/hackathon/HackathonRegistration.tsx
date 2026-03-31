'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useAuth } from '@/hooks/useAuth';
import {
  useHackathonRegistrations,
  useRegisterForHackathon,
  useLinkAgentToHackathon,
} from '@/hooks/useHackathons';
import { useAgents } from '@/hooks';
import { useToast } from '@/components/ui/Toast';
import type { Hackathon } from '@/types';

function extractErrorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  return 'An unexpected error occurred';
}

interface HackathonRegistrationProps {
  hackathon: Hackathon;
}

export function HackathonRegistration({ hackathon }: HackathonRegistrationProps) {
  const { address, isConnected, connect } = useBaseWallet();
  const { isAuthenticated, login } = useAuth();
  const { addToast } = useToast();
  const [agentId, setAgentId] = useState('');

  const { data: registrationsData } = useHackathonRegistrations(hackathon.id);
  const registerMutation = useRegisterForHackathon();
  const linkAgentMutation = useLinkAgentToHackathon();
  const { data: agentsData } = useAgents({});

  const currentWallet = address?.toLowerCase();
  const isRegistered = registrationsData?.registrations?.some(
    (r) => r.walletAddress.toLowerCase() === currentWallet,
  );

  const canRegister =
    hackathon.status === 'upcoming' || hackathon.status === 'active';

  const handleRegister = async () => {
    try {
      await registerMutation.mutateAsync({ hackathonId: hackathon.id });
      addToast('Registered for hackathon', 'success');
    } catch (err: unknown) {
      addToast(extractErrorMessage(err), 'error');
    }
  };

  const handleLinkAgent = async () => {
    if (!agentId.trim()) return;
    try {
      await linkAgentMutation.mutateAsync({
        hackathonId: hackathon.id,
        agentId: agentId.trim(),
      });
      addToast('Agent linked to hackathon', 'success');
      setAgentId('');
    } catch (err: unknown) {
      addToast(extractErrorMessage(err), 'error');
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Registration</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Step 1: Connect wallet */}
        {!isConnected && (
          <div className="space-y-2">
            <p className="text-sm text-text-secondary">
              Connect your wallet to get started.
            </p>
            <Button onClick={connect}>Connect Wallet</Button>
          </div>
        )}

        {/* Step 2: Sign in */}
        {isConnected && !isAuthenticated && (
          <div className="space-y-2">
            <p className="text-sm text-text-secondary">
              Sign in with your wallet to register.
            </p>
            <Button onClick={login}>Sign In</Button>
          </div>
        )}

        {/* Step 3: Register */}
        {isConnected && isAuthenticated && !isRegistered && canRegister && (
          <div className="space-y-2">
            <p className="text-sm text-text-secondary">
              Register your wallet to participate. Create your agents using the{' '}
              <code className="px-1 py-0.5 bg-bg-tertiary text-xs">r44</code>{' '}
              CLI or the Agents dashboard, then link them below.
            </p>
            <Button
              onClick={handleRegister}
              disabled={registerMutation.isPending}
            >
              {registerMutation.isPending ? 'Registering...' : 'Register'}
            </Button>
          </div>
        )}

        {/* Step 4: Link agents */}
        {isConnected && isAuthenticated && isRegistered && (
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <Badge variant="bid">Registered</Badge>
              <span className="text-sm text-text-secondary">
                You&apos;re in! Link your trading agents below.
              </span>
            </div>

            <div className="flex gap-2">
              <div className="flex-1">
                <label htmlFor="hackathon-agent-id" className="sr-only">
                  Agent ID
                </label>
                <input
                  id="hackathon-agent-id"
                  type="text"
                  value={agentId}
                  onChange={(e) => setAgentId(e.target.value)}
                  placeholder="Agent ID (on-chain)"
                  aria-label="On-chain agent ID to link"
                  className="w-full px-3 py-2 text-sm bg-bg-secondary border border-border focus:border-accent focus:outline-none transition-colors"
                />
              </div>
              <Button
                onClick={handleLinkAgent}
                disabled={linkAgentMutation.isPending || !agentId.trim()}
              >
                {linkAgentMutation.isPending ? 'Linking...' : 'Link Agent'}
              </Button>
            </div>

            <p className="text-xs text-text-muted">
              Create agents via{' '}
              <code className="px-1 py-0.5 bg-bg-tertiary">r44 agent create</code>{' '}
              or the <a href="/agents" className="text-accent hover:underline">Agents</a> page,
              then paste the on-chain agent ID here. Max 3 agents per wallet.
            </p>
          </div>
        )}

        {/* Not open for registration */}
        {!canRegister && !isRegistered && (
          <p className="text-sm text-text-muted">
            Registration is closed for this hackathon.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
