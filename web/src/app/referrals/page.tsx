'use client';

import { useState } from 'react';
import { PageShell } from '@/components/layout';
import { Card, Button, Input, Badge, Spinner } from '@/components/ui';
import {
  useReferralCode,
  useReferralStats,
  useGenerateReferralCode,
  useApplyReferralCode,
} from '@/hooks';
import { useAuth } from '@/hooks/useAuth';
import { truncateAddress } from '@/lib/utils';

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback for older browsers
      const textarea = document.createElement('textarea');
      textarea.value = text;
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <Button variant="secondary" size="sm" onClick={handleCopy}>
      {copied ? 'Copied!' : 'Copy'}
    </Button>
  );
}

function ReferralLinkSection() {
  const { data: codeData, isLoading: codeLoading } = useReferralCode();
  const generateCode = useGenerateReferralCode();

  const code = codeData?.code;
  const referralLink = code
    ? `${typeof window !== 'undefined' ? window.location.origin : ''}?ref=${code}`
    : null;

  return (
    <Card className="mb-6">
      <h2 className="font-semibold mb-4">Your Referral Link</h2>

      {codeLoading ? (
        <div className="flex items-center justify-center py-4">
          <Spinner size="sm" />
        </div>
      ) : code ? (
        <div className="space-y-3">
          <div className="flex flex-col gap-2">
            <span className="text-text-secondary text-sm">Referral Code</span>
            <div className="flex items-center gap-2">
              <code className="flex-1 bg-bg-secondary px-3 py-2 font-mono text-sm break-all">
                {code}
              </code>
              <CopyButton text={code} />
            </div>
          </div>
          {referralLink && (
            <div className="flex flex-col gap-2">
              <span className="text-text-secondary text-sm">Shareable Link</span>
              <div className="flex items-center gap-2">
                <code className="flex-1 bg-bg-secondary px-3 py-2 font-mono text-xs break-all">
                  {referralLink}
                </code>
                <CopyButton text={referralLink} />
              </div>
            </div>
          )}
        </div>
      ) : (
        <div className="space-y-3">
          <p className="text-text-secondary text-sm">
            Generate a referral code to start inviting others and earning rewards.
          </p>
          <Button
            onClick={() => generateCode.mutate()}
            disabled={generateCode.isPending}
          >
            {generateCode.isPending ? 'Generating...' : 'Generate Referral Code'}
          </Button>
          {generateCode.isError && (
            <p className="text-red-400 text-sm">
              {generateCode.error?.message || 'Failed to generate code'}
            </p>
          )}
        </div>
      )}
    </Card>
  );
}

function ApplyCodeSection() {
  const [inputCode, setInputCode] = useState('');
  const [validationError, setValidationError] = useState('');
  const applyCode = useApplyReferralCode();

  const handleApply = () => {
    const code = inputCode.trim();
    if (!code) return;
    if (code.length < 1 || code.length > 32) {
      setValidationError('Code must be between 1 and 32 characters.');
      return;
    }
    if (!/^[a-f0-9]+$/i.test(code)) {
      setValidationError('Code must be alphanumeric (hex characters only).');
      return;
    }
    setValidationError('');
    applyCode.mutate(code);
  };

  return (
    <Card className="mb-6">
      <h2 className="font-semibold mb-4">Apply a Referral Code</h2>
      <p className="text-text-secondary text-sm mb-4">
        Were you invited by someone? Enter their referral code below.
      </p>
      <div className="flex gap-2">
        <Input
          placeholder="Enter referral code"
          value={inputCode}
          onChange={(e) => {
            setInputCode(e.target.value);
            if (validationError) setValidationError('');
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') handleApply();
          }}
          className="flex-1"
        />
        <Button
          onClick={handleApply}
          disabled={applyCode.isPending || !inputCode.trim()}
        >
          {applyCode.isPending ? 'Applying...' : 'Apply'}
        </Button>
      </div>
      {validationError && (
        <p className="text-red-400 text-sm mt-2">{validationError}</p>
      )}
      {applyCode.isSuccess && (
        <p className="text-green-400 text-sm mt-2">
          Referral code applied successfully!
        </p>
      )}
      {applyCode.isError && (
        <p className="text-red-400 text-sm mt-2">
          {applyCode.error?.message || 'Failed to apply code'}
        </p>
      )}
    </Card>
  );
}

function StatsSection() {
  const { data: stats, isLoading } = useReferralStats();

  if (isLoading) {
    return (
      <Card className="mb-6">
        <h2 className="font-semibold mb-4">Referral Stats</h2>
        <div className="flex items-center justify-center py-4">
          <Spinner size="sm" />
        </div>
      </Card>
    );
  }

  if (!stats) return null;

  return (
    <Card className="mb-6">
      <h2 className="font-semibold mb-4">Referral Stats</h2>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-6">
        <div className="bg-bg-secondary p-4">
          <div className="text-text-secondary text-xs uppercase tracking-wider mb-1">
            Total Referrals
          </div>
          <div className="text-2xl font-bold">{stats.totalReferrals}</div>
        </div>
        <div className="bg-bg-secondary p-4">
          <div className="text-text-secondary text-xs uppercase tracking-wider mb-1">
            Rewarded
          </div>
          <div className="text-2xl font-bold text-green-400">
            {stats.rewardedCount}
          </div>
        </div>
        <div className="bg-bg-secondary p-4">
          <div className="text-text-secondary text-xs uppercase tracking-wider mb-1">
            Pending Rewards
          </div>
          <div className="text-2xl font-bold text-yellow-400">
            {stats.pendingRewards}
          </div>
        </div>
      </div>

      {stats.referredBy && (
        <div className="mb-4 text-sm text-text-secondary">
          You were referred by{' '}
          <span className="font-mono text-text-primary">
            {truncateAddress(stats.referredBy)}
          </span>
        </div>
      )}

      {stats.referees.length > 0 && (
        <div>
          <h3 className="text-sm font-medium mb-2 text-text-secondary uppercase tracking-wider">
            Your Referees
          </h3>
          <div className="divide-y divide-border">
            {stats.referees.map((referee) => (
              <div
                key={referee.wallet}
                className="flex items-center justify-between py-2 text-sm"
              >
                <span className="font-mono">
                  {truncateAddress(referee.wallet)}
                </span>
                <div className="flex items-center gap-3">
                  <span className="text-text-secondary text-xs">
                    {new Date(referee.createdAt).toLocaleDateString()}
                  </span>
                  <Badge variant={referee.rewarded ? 'default' : 'secondary'}>
                    {referee.rewarded ? 'Rewarded' : 'Pending'}
                  </Badge>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </Card>
  );
}

export default function ReferralsPage() {
  const { isAuthenticated } = useAuth();

  return (
    <PageShell>
      <div className="py-8">
        <div className="mx-auto max-w-3xl">
          <h1 className="text-2xl font-bold text-text-primary mb-2">Referrals</h1>
          <p className="mb-6 text-sm leading-6 text-text-secondary">
            Invite friends to Relay44 and earn rewards when they start trading.
            Share your unique referral link and track your progress below.
          </p>

          {!isAuthenticated ? (
            <Card>
              <p className="text-text-secondary text-center py-8">
                Connect your wallet to access the referral program.
              </p>
            </Card>
          ) : (
            <>
              <ReferralLinkSection />
              <ApplyCodeSection />
              <StatsSection />
            </>
          )}
        </div>
      </div>
    </PageShell>
  );
}
