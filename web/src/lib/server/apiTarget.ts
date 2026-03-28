function normalizeTarget(raw: string | undefined): string {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) {
    return '';
  }

  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }

  return `http://${trimmed}`;
}

export function resolveApiProxyTarget(): string {
  const explicit = process.env.API_PROXY_TARGET || process.env.NEXT_PUBLIC_API_URL;
  if (explicit?.trim()) {
    return normalizeTarget(explicit);
  }

  if (process.env.NODE_ENV !== 'production') {
    return 'http://localhost:8080/v1';
  }

  return '';
}

export function hasApiProxyTarget(): boolean {
  return Boolean(resolveApiProxyTarget());
}
