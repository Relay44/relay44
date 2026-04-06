'use client';

import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import type { GraphNode, VerificationStatus } from './types';

interface Props {
  claim: GraphNode;
}

const STATUS_LABELS: Record<VerificationStatus, string> = {
  unverified: 'Unverified',
  supported: 'Supported',
  disputed: 'Disputed',
  debunked: 'Debunked',
};

const STATUS_VARIANTS: Record<VerificationStatus, 'muted' | 'success' | 'warning' | 'danger'> = {
  unverified: 'muted',
  supported: 'success',
  disputed: 'warning',
  debunked: 'danger',
};

export function ClaimCard({ claim }: Props) {
  const status = (claim.data.verificationStatus as VerificationStatus) || 'unverified';
  const confidence = (claim.data.confidence as number) || 0;
  const sentiment = (claim.data.sentiment as number) || 0;

  return (
    <Card className="p-3">
      <div className="flex items-start justify-between gap-2 mb-2">
        <p className="text-sm text-text-primary flex-1">
          {claim.data.text as string || claim.label}
        </p>
        <Badge variant={STATUS_VARIANTS[status]} className="shrink-0">
          {STATUS_LABELS[status]}
        </Badge>
      </div>

      <div className="flex items-center gap-3 text-xs text-text-muted">
        <span>Confidence: {confidence}%</span>
        <span>
          Sentiment:{' '}
          <span className={sentiment > 0 ? 'text-bid' : sentiment < 0 ? 'text-ask' : ''}>
            {sentiment > 0 ? '+' : ''}{sentiment.toFixed(2)}
          </span>
        </span>
      </div>
    </Card>
  );
}
