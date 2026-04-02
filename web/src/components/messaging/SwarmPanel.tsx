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

export function SwarmPanel({ swarmId, className }: SwarmPanelProps) {
  const [messages, setMessages] = useState<SwarmMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const prevMessageCountRef = useRef(0);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  const fetchMessages = useCallback(async () => {
    try {
      const response = await api.getSwarmMessages(swarmId, { limit: 50 });
      setMessages(response.data);
      setError(null);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to load messages';
      if (loading) {
        setError(message);
      }
    } finally {
      setLoading(false);
    }
  }, [loading, swarmId]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    setMessages([]);
    fetchMessages();
  }, [fetchMessages, swarmId]);

  useEffect(() => {
    const interval = setInterval(fetchMessages, 5000);
    return () => clearInterval(interval);
  }, [fetchMessages]);

  useEffect(() => {
    if (messages.length > prevMessageCountRef.current) {
      scrollToBottom();
    }
    prevMessageCountRef.current = messages.length;
  }, [messages.length, scrollToBottom]);

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

  if (error) {
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
            fetchMessages();
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
