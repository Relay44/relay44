'use client';

import { useState, useEffect, useRef, useCallback } from 'react';
import { Button, Input, Spinner } from '@/components/ui';
import { useToast } from '@/components/ui/Toast';
import { useBaseWallet } from '@/hooks/useBaseWallet';
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
  if (address.length <= 10) return address;
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
  const { address } = useBaseWallet();
  const { addToast } = useToast();

  const [messages, setMessages] = useState<SwarmMessage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [input, setInput] = useState('');
  const [sending, setSending] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const prevMessageCountRef = useRef(0);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, []);

  const fetchMessages = useCallback(async () => {
    try {
      const res = await api.getSwarmMessages(swarmId, { limit: 50 });
      setMessages(res.data);
      setError(null);
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to load messages';
      if (loading) {
        setError(msg);
      }
    } finally {
      setLoading(false);
    }
  }, [swarmId, loading]);

  // Initial fetch
  useEffect(() => {
    setLoading(true);
    setError(null);
    setMessages([]);
    fetchMessages();
  }, [swarmId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Poll every 5 seconds
  useEffect(() => {
    const interval = setInterval(fetchMessages, 5000);
    return () => clearInterval(interval);
  }, [fetchMessages]);

  // Scroll to bottom on new messages
  useEffect(() => {
    if (messages.length > prevMessageCountRef.current) {
      scrollToBottom();
    }
    prevMessageCountRef.current = messages.length;
  }, [messages.length, scrollToBottom]);

  const handleSend = async () => {
    const trimmed = input.trim();
    if (!trimmed || sending) return;

    setSending(true);
    try {
      await api.sendSwarmMessage(swarmId, trimmed);
      setInput('');
      // Immediately refresh to show the new message
      await fetchMessages();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to send message';
      addToast(msg, 'error');
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const connectedAddress = address?.toLowerCase();

  if (loading) {
    return (
      <div className={cn('flex flex-col h-full items-center justify-center', className)}>
        <Spinner />
        <p className="mt-3 text-sm text-text-secondary">Loading messages...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className={cn('flex flex-col h-full items-center justify-center', className)}>
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
    <div className={cn('flex flex-col h-full', className)}>
      {/* Header */}
      <div className="px-4 py-3 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <h2 className="text-sm font-medium text-text-primary truncate">
            Swarm
          </h2>
          <span className="text-xs font-mono text-text-secondary truncate">
            {truncateAddress(swarmId)}
          </span>
        </div>
        <span className="text-xs text-text-secondary shrink-0">
          {messages.length} message{messages.length !== 1 ? 's' : ''}
        </span>
      </div>

      {/* Messages */}
      <div
        ref={messagesContainerRef}
        className="flex-1 overflow-y-auto px-4 py-3 space-y-3"
      >
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <p className="text-sm text-text-secondary">
              No messages yet. Start the conversation.
            </p>
          </div>
        ) : (
          messages.map((msg) => {
            const isOwn = connectedAddress
              ? msg.sender.toLowerCase() === connectedAddress
              : false;

            return (
              <div
                key={msg.id}
                className={cn(
                  'px-3 py-2 rounded',
                  isOwn ? 'bg-accent/10 ml-8' : 'bg-bg-secondary mr-8',
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-mono text-text-secondary">
                    {isOwn ? 'You' : truncateAddress(msg.sender)}
                  </span>
                  <span className="text-xs text-text-secondary shrink-0">
                    {relativeTime(msg.sentAt)}
                  </span>
                </div>
                <p className="text-sm text-text-primary mt-1 whitespace-pre-wrap break-words">
                  {msg.content}
                </p>
              </div>
            );
          })
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="px-4 py-3 border-t border-border flex gap-2">
        <Input
          className="flex-1"
          placeholder="Type a message..."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={sending}
        />
        <Button
          variant="primary"
          size="sm"
          onClick={handleSend}
          disabled={!input.trim() || sending}
          loading={sending}
        >
          Send
        </Button>
      </div>
    </div>
  );
}
