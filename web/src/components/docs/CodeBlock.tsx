'use client';

import { useCallback, useState } from 'react';

interface CodeBlockProps {
  code: string;
  language?: string;
}

export function CodeBlock({ code, language = 'bash' }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const copy = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [code]);

  return (
    <div className="group relative border border-border">
      <div className="flex items-center justify-between border-b border-border bg-bg-secondary px-4 py-2">
        <span className="text-[0.7rem] uppercase tracking-widest text-text-muted">{language}</span>
        <button
          onClick={copy}
          className="text-[0.7rem] uppercase tracking-widest text-text-muted transition-colors hover:text-text-primary"
        >
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre className="overflow-x-auto p-4 text-sm leading-relaxed text-text-primary">
        <code>{code}</code>
      </pre>
    </div>
  );
}
