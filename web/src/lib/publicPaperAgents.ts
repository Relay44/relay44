import type { ExternalAgentRecord } from '@/lib/api';

type PublicPaperAgentNameInput = Pick<
  ExternalAgentRecord,
  'id' | 'name' | 'provider' | 'outcome' | 'side' | 'strategy_label'
>;

const strategyRoles: Record<string, { title: string; roles: string[] }> = {
  proving: {
    title: 'Proving',
    roles: ['Scout', 'Signal', 'Trace', 'Vector', 'Pulse', 'Lens'],
  },
  research: {
    title: 'Research',
    roles: ['Atlas', 'Survey', 'Scope', 'Frame', 'Ledger', 'Index'],
  },
  optimization: {
    title: 'Optimization',
    roles: ['Mesh', 'Maker', 'Engine', 'Grid', 'Node', 'Desk'],
  },
};

const providerOffsets: Record<ExternalAgentRecord['provider'], number> = {
  limitless: 1,
  polymarket: 3,
  aerodrome: 5,
};

function hashValue(input: string): number {
  let hash = 0;

  for (const char of input) {
    hash = (hash * 33 + char.charCodeAt(0)) >>> 0;
  }

  return hash;
}

function extractSerial(name: string): string | null {
  const match = name.match(/(\d+)(?!.*\d)/);
  if (!match) {
    return null;
  }

  return String(Number(match[1])).padStart(2, '0');
}

function titleCase(value: string): string {
  return value
    .split(/[\s-]+/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1).toLowerCase())
    .join(' ');
}

export function formatPublicPaperAgentName(
  agent: PublicPaperAgentNameInput,
): string {
  const strategyKey = agent.strategy_label.trim().toLowerCase();
  const profile = strategyRoles[strategyKey] ?? {
    title: titleCase(agent.strategy_label || 'Paper'),
    roles: ['Signal', 'Trace', 'Vector', 'Grid', 'Node', 'Desk'],
  };
  const seed = hashValue(
    [agent.id, agent.name, agent.provider, agent.outcome, agent.side].join(':'),
  );
  const role =
    profile.roles[(seed + providerOffsets[agent.provider]) % profile.roles.length];
  const serial = extractSerial(agent.name);

  return serial
    ? `${profile.title} ${role} ${serial}`
    : `${profile.title} ${role}`;
}
