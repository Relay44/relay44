'use client';

import { useCallback, useEffect, useRef, useState } from 'react';

import { Button, Spinner } from '@/components/ui';
import { api } from '@/lib/api';
import { cn } from '@/lib/utils';

interface SwarmMessage {
  id: string;
  sender: string;
  content: string;
  sentAt: string;
}

interface SwarmPanelProps {
  swarmId: string;
  className?: string;
}

const PAGE_SIZE = 50;
const MIN_POLL_MS = 5_000;
const MAX_POLL_MS = 60_000;

function truncateAddress(address: string): string {
  if (address.length <= 10) {
    return address;
  }
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

function relativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diff = Math.max(0, now - then);
  const seconds = Math.floor(diff / 1000);

  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(iso).toLocaleDateString();
}

function mergeMessages(
  existing: SwarmMessage[],
  incoming: SwarmMessage[],
): SwarmMessage[] {
  const seen = new Set<string>();
  const merged: SwarmMessage[] = [];
  for (const msg of [...incoming, ...existing]) {
    if (seen.has(msg.id)) continue;
    seen.add(msg.id);
    merged.push(msg);
  }
  merged.sort((a, b) => {
    const ta = new Date(a.sentAt).getTime();
    const tb = new Date(b.sentAt).getTime();
    if (ta !== tb) return ta - tb;
    // Deterministic tiebreaker when timestamps collide.
    if (a.id === b.id) return 0;
    return a.id < b.id ? -1 : 1;
  });
  return merged;
}

export function SwarmPanel({ swarmId, className }: SwarmPanelProps) {
  const [messages, setMessages] = useState<SwarmMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [hasMoreOlder, setHasMoreOlder] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesStartRef = useRef<HTMLDivElement>(null);
  const prevMessageCountRef = useRef(0);
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const consecutiveErrorsRef = useRef(0);
  const offsetRef = useRef(PAGE_SIZE);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  const fetchLatest = useCallback(async () => {
    try {
      const response = await api.getSwarmMessages(swarmId, {
        limit: PAGE_SIZE,
        offset: 0,
      });
      setMessages((prev) => mergeMessages(prev, response.data));
      setHasMoreOlder(response.has_more);
      setError(null);
      consecutiveErrorsRef.current = 0;
      return true;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to load messages';
      consecutiveErrorsRef.current += 1;
      setError((prev) => (prev == null && !loading ? null : message));
      return false;
    } finally {
      setLoading(false);
    }
  }, [loading, swarmId]);

  const loadOlder = useCallback(async () => {
    if (loadingOlder || !hasMoreOlder) return;
    setLoadingOlder(true);
    try {
      const response = await api.getSwarmMessages(swarmId, {
        limit: PAGE_SIZE,
        offset: offsetRef.current,
      });
      setMessages((prev) => mergeMessages(prev, response.data));
      offsetRef.current += response.data.length;
      setHasMoreOlder(response.has_more && response.data.length > 0);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to load older messages';
      setError(message);
    } finally {
      setLoadingOlder(false);
    }
  }, [hasMoreOlder, loadingOlder, swarmId]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    setMessages([]);
    setHasMoreOlder(false);
    offsetRef.current = PAGE_SIZE;
    consecutiveErrorsRef.current = 0;
    fetchLatest();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [swarmId]);

  useEffect(() => {
    let cancelled = false;

    const clearTimer = () => {
      if (pollTimerRef.current) {
        clearTimeout(pollTimerRef.current);
        pollTimerRef.current = null;
      }
    };

    const schedule = () => {
      if (cancelled) return;
      if (typeof document !== 'undefined' && document.hidden) return;
      const errors = consecutiveErrorsRef.current;
      const delay =
        errors === 0
          ? MIN_POLL_MS
          : Math.min(MIN_POLL_MS * 2 ** errors, MAX_POLL_MS);
      pollTimerRef.current = setTimeout(async () => {
        await fetchLatest();
        schedule();
      }, delay);
    };

    const handleVisibility = () => {
      if (typeof document === 'undefined') return;
      if (document.hidden) {
        clearTimer();
      } else {
        clearTimer();
        // On refocus, reset backoff and fetch immediately.
        consecutiveErrorsRef.current = 0;
        void fetchLatest().then(() => schedule());
      }
    };

    schedule();
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', handleVisibility);
    }

    return () => {
      cancelled = true;
      clearTimer();
      if (typeof document !== 'undefined') {
        document.removeEventListener('visibilitychange', handleVisibility);
      }
    };
  }, [fetchLatest]);

  useEffect(() => {
    if (messages.length > prevMessageCountRef.current && !loadingOlder) {
      scrollToBottom();
    }
    prevMessageCountRef.current = messages.length;
  }, [messages.length, loadingOlder, scrollToBottom]);

  if (loading) {
    return (
      <div
        className={cn(
          'flex h-full flex-col items-center justify-center',
          className,
        )}
      >
        <Spinner />
        <p className="mt-3 text-sm text-text-secondary">Loading messages...</p>
      </div>
    );
  }

  if (error && messages.length === 0) {
    return (
      <div
        className={cn(
          'flex h-full flex-col items-center justify-center',
          className,
        )}
      >
        <p className="text-sm text-text-secondary">{error}</p>
        <Button
          variant="secondary"
          size="sm"
          className="mt-3"
          onClick={() => {
            setLoading(true);
            setError(null);
            consecutiveErrorsRef.current = 0;
            fetchLatest();
          }}
        >
          Retry
        </Button>
      </div>
    );
  }

  return (
    <div className={cn('flex h-full flex-col', className)}>
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <h2 className="truncate text-sm font-medium text-text-primary">
            Swarm
          </h2>
          <span className="truncate font-mono text-xs text-text-secondary">
            {truncateAddress(swarmId)}
          </span>
        </div>
        <span className="shrink-0 text-xs text-text-secondary">
          {messages.length} message{messages.length !== 1 ? 's' : ''}
        </span>
      </div>

      <div className="flex-1 space-y-3 overflow-y-auto px-4 py-3">
        <div ref={messagesStartRef} />
        {hasMoreOlder && (
          <div className="flex justify-center">
            <Button
              variant="secondary"
              size="sm"
              disabled={loadingOlder}
              onClick={loadOlder}
            >
              {loadingOlder ? 'Loading…' : 'Load older'}
            </Button>
          </div>
        )}
        {error && messages.length > 0 && (
          <div className="rounded border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-400">
            {error}
          </div>
        )}
        {messages.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <p className="text-sm text-text-secondary">No messages yet.</p>
          </div>
        ) : (
          messages.map((message) => (
            <div key={message.id} className="mr-8 rounded bg-bg-secondary px-3 py-2">
              <div className="flex items-center justify-between gap-2">
                <span className="font-mono text-xs text-text-secondary">
                  {truncateAddress(message.sender)}
                </span>
                <span className="shrink-0 text-xs text-text-secondary">
                  {relativeTime(message.sentAt)}
                </span>
              </div>
              <p className="mt-1 break-words whitespace-pre-wrap text-sm text-text-primary">
                {message.content}
              </p>
            </div>
          ))
        )}
        <div ref={messagesEndRef} />
      </div>

      <div className="border-t border-border px-4 py-3">
        <p className="text-sm text-text-secondary">
          Browser sending is disabled. Swarm posting still depends on a
          server-side signing key, so this page is read-only for now.
        </p>
      </div>
    </div>
  );
}
