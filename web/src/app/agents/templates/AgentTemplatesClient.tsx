'use client';

import Link from 'next/link';
import { useMemo, useState } from 'react';
import { Bot, Shield, TrendingUp, Zap } from 'lucide-react';
import { PageShell } from '@/components/layout';
import { Button, Card, Badge, Spinner } from '@/components/ui';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/Select';
import { useAgentTemplates } from '@/hooks';
import { formatCurrency } from '@/lib/utils';
import type { AgentTemplate } from '@/types';

const RISK_COLORS: Record<string, 'success' | 'warning' | 'danger'> = {
  low: 'success',
  medium: 'warning',
  high: 'danger',
};

function riskIcon(tier: string) {
  if (tier === 'low') return <Shield className="h-3.5 w-3.5" />;
  if (tier === 'medium') return <TrendingUp className="h-3.5 w-3.5" />;
  return <Zap className="h-3.5 w-3.5" />;
}

function TemplateCard({ template }: { template: AgentTemplate }) {
  const riskVariant = RISK_COLORS[template.riskTier] ?? 'default';

  return (
    <Card hover className="flex flex-col gap-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="flex h-9 w-9 items-center justify-center bg-bg-tertiary border border-border">
            <Bot className="h-4.5 w-4.5 text-accent" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-text-primary leading-tight">
              {template.name}
            </h3>
            <p className="text-xs text-text-muted">{template.strategy}</p>
          </div>
        </div>
        <Badge variant={riskVariant} className="shrink-0 flex items-center gap-1">
          {riskIcon(template.riskTier)}
          {template.riskTier}
        </Badge>
      </div>

      {template.description && (
        <p className="text-xs text-text-secondary leading-relaxed line-clamp-2">
          {template.description}
        </p>
      )}

      <div className="grid grid-cols-2 gap-3 text-xs">
        <div>
          <span className="text-text-muted">Category</span>
          <p className="text-text-primary font-medium capitalize">{template.category}</p>
        </div>
        <div>
          <span className="text-text-muted">Min seed</span>
          <p className="text-text-primary font-medium">
            {formatCurrency(template.minSeedUsdc)}
          </p>
        </div>
      </div>

      <Link href={`/agents/deploy/${template.id}`} className="mt-auto">
        <Button variant="primary" size="sm" className="w-full">
          Deploy
        </Button>
      </Link>
    </Card>
  );
}

export function AgentTemplatesClient() {
  const { data: templates, isLoading } = useAgentTemplates();
  const [categoryFilter, setCategoryFilter] = useState<string>('all');
  const [riskFilter, setRiskFilter] = useState<string>('all');

  const categories = useMemo(() => {
    if (!templates) return [];
    const cats = [...new Set(templates.map((t) => t.category))];
    return cats.sort();
  }, [templates]);

  const riskTiers = useMemo(() => {
    if (!templates) return [];
    return [...new Set(templates.map((t) => t.riskTier))].sort();
  }, [templates]);

  const filtered = useMemo(() => {
    if (!templates) return [];
    return templates.filter((t) => {
      if (categoryFilter !== 'all' && t.category !== categoryFilter) return false;
      if (riskFilter !== 'all' && t.riskTier !== riskFilter) return false;
      return true;
    });
  }, [templates, categoryFilter, riskFilter]);

  return (
    <PageShell>
      <div className="space-y-6">
        <div className="flex flex-col gap-1">
          <h1 className="text-lg font-bold text-text-primary">Agent templates</h1>
          <p className="text-sm text-text-secondary">
            Browse pre-built strategies and deploy a managed agent in minutes.
          </p>
        </div>

        <div className="flex flex-wrap gap-3">
          <Select value={categoryFilter} onValueChange={setCategoryFilter}>
            <SelectTrigger className="w-[160px] h-9 text-xs">
              <SelectValue placeholder="Category" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All categories</SelectItem>
              {categories.map((c) => (
                <SelectItem key={c} value={c} className="capitalize">
                  {c}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <Select value={riskFilter} onValueChange={setRiskFilter}>
            <SelectTrigger className="w-[140px] h-9 text-xs">
              <SelectValue placeholder="Risk" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All risk</SelectItem>
              {riskTiers.map((r) => (
                <SelectItem key={r} value={r} className="capitalize">
                  {r}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {isLoading ? (
          <div className="flex items-center justify-center py-16">
            <Spinner />
          </div>
        ) : filtered.length === 0 ? (
          <div className="text-center py-16">
            <p className="text-sm text-text-muted">
              {templates?.length === 0
                ? 'No agent templates available yet.'
                : 'No templates match your filters.'}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {filtered.map((t) => (
              <TemplateCard key={t.id} template={t} />
            ))}
          </div>
        )}
      </div>
    </PageShell>
  );
}
