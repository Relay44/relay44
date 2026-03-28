'use client';

import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
import { PageShell } from '@/components/layout';
import { Button, Card, Input, Select, useToast } from '@/components/ui';
import { ReadOnlyNotice } from '@/components/runtime/ReadOnlyNotice';
import { useBaseWallet } from '@/hooks/useBaseWallet';
import { useAuth, useRuntimeMode, useSessionState } from '@/hooks';
import { api, type ExternalCredential, type ExternalCredentialStatus } from '@/lib/api';

type Provider = 'limitless' | 'polymarket';

interface DraftState {
  provider: Provider;
  label: string;
  apiKey: string;
  apiSecret: string;
  apiPassphrase: string;
  baseWallet: string;
  funder: string;
  signatureType: string;
  defaultSignedOrder: string;
}

const EMPTY_DRAFT: DraftState = {
  provider: 'limitless',
  label: '',
  apiKey: '',
  apiSecret: '',
  apiPassphrase: '',
  baseWallet: '',
  funder: '',
  signatureType: '0',
  defaultSignedOrder: '',
};

function parseSignedOrder(raw: string) {
  const trimmed = raw.trim();
  if (!trimmed) {
    return undefined;
  }
  return JSON.parse(trimmed) as Record<string, unknown>;
}

function buildPayload(draft: DraftState) {
  const payload: Record<string, unknown> = {
    apiKey: draft.apiKey.trim(),
  };

  if (draft.provider === 'polymarket') {
    payload.apiSecret = draft.apiSecret.trim();
    payload.apiPassphrase = draft.apiPassphrase.trim();
    payload.funder = draft.funder.trim();
    payload.signatureType = Number(draft.signatureType || '0');
  }

  if (draft.provider === 'limitless' && draft.baseWallet.trim()) {
    payload.baseWallet = draft.baseWallet.trim();
  }

  const defaultSignedOrder = parseSignedOrder(draft.defaultSignedOrder);
  if (defaultSignedOrder) {
    payload.defaultSignedOrder = defaultSignedOrder;
  }

  return payload;
}

export default function ExternalCredentialsPage() {
  const { addToast } = useToast();
  const { readOnly } = useRuntimeMode();
  const baseWallet = useBaseWallet();
  const { hasSession, sessionRestored } = useSessionState();
  const { login, isLoading: isAuthenticating, isAuthenticated, walletConnected } = useAuth();
  const [draft, setDraft] = useState<DraftState>(EMPTY_DRAFT);
  const [credentials, setCredentials] = useState<ExternalCredential[]>([]);
  const [selectedCredentialId, setSelectedCredentialId] = useState('');
  const [credentialStatus, setCredentialStatus] = useState<ExternalCredentialStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [binding, setBinding] = useState(false);

  const canManage = sessionRestored && (hasSession || isAuthenticated);

  const visibleCredentials = useMemo(
    () => credentials.filter((entry) => entry.provider === draft.provider),
    [credentials, draft.provider],
  );

  useEffect(() => {
    if (!canManage || readOnly) {
      setCredentials([]);
      return;
    }

    let cancelled = false;

    async function load() {
      setLoading(true);
      try {
        const [limitless, polymarket] = await Promise.all([
          api.getExternalCredentials('limitless'),
          api.getExternalCredentials('polymarket'),
        ]);
        if (cancelled) return;
        const merged = [...limitless, ...polymarket];
        setCredentials(merged);
      } catch (error) {
        if (!cancelled) {
          const message = error instanceof Error ? error.message : 'Failed to load credentials';
          addToast(message, 'error');
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [addToast, canManage, readOnly]);

  async function refreshCredentials() {
    const [limitless, polymarket] = await Promise.all([
      api.getExternalCredentials('limitless'),
      api.getExternalCredentials('polymarket'),
    ]);
    setCredentials([...limitless, ...polymarket]);
  }

  useEffect(() => {
    const nextSelected = visibleCredentials.find((entry) => entry.id === selectedCredentialId)?.id
      ?? visibleCredentials[0]?.id
      ?? '';
    if (nextSelected !== selectedCredentialId) {
      setSelectedCredentialId(nextSelected);
    }
  }, [selectedCredentialId, visibleCredentials]);

  useEffect(() => {
    if (!canManage || readOnly) {
      setCredentialStatus(null);
      return;
    }

    let cancelled = false;

    async function loadStatus() {
      try {
        const status = await api.getExternalCredentialStatus(
          draft.provider,
          selectedCredentialId || undefined,
        );
        if (!cancelled) {
          setCredentialStatus(status);
        }
      } catch (error) {
        if (!cancelled) {
          const message =
            error instanceof Error ? error.message : 'Failed to load credential readiness';
          addToast(message, 'error');
        }
      }
    }

    void loadStatus();

    return () => {
      cancelled = true;
    };
  }, [addToast, canManage, draft.provider, readOnly, selectedCredentialId]);

  async function handleSave(event: React.FormEvent) {
    event.preventDefault();

    if (readOnly) {
      addToast('Credential changes are unavailable in this environment', 'error');
      return;
    }
    if (!canManage) {
      addToast('Authenticate before saving credentials', 'error');
      return;
    }
    if (!draft.apiKey.trim()) {
      addToast('API key is required', 'error');
      return;
    }
    if (draft.provider === 'polymarket' && (!draft.apiSecret.trim() || !draft.apiPassphrase.trim())) {
      addToast('Polymarket requires apiSecret and apiPassphrase', 'error');
      return;
    }
    if (draft.provider === 'polymarket' && !draft.funder.trim()) {
      addToast('Polymarket requires a funder wallet', 'error');
      return;
    }

    setSaving(true);
    try {
      await api.upsertExternalCredential({
        provider: draft.provider,
        label: draft.label.trim() || `${draft.provider}-credential`,
        credentials: buildPayload(draft),
      });
      await refreshCredentials();
      setDraft((current) => ({ ...EMPTY_DRAFT, provider: current.provider }));
      addToast('Credential saved', 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Credential save failed';
      addToast(message, 'error');
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(credentialId: string) {
    if (readOnly) {
      addToast('Credential changes are unavailable in this environment', 'error');
      return;
    }

    try {
      await api.deleteExternalCredential(credentialId);
      await refreshCredentials();
      if (selectedCredentialId === credentialId) {
        setSelectedCredentialId('');
      }
      addToast('Credential removed', 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Credential delete failed';
      addToast(message, 'error');
    }
  }

  async function handleBindWallet() {
    if (!selectedCredentialId) {
      addToast('Select a limitless credential first', 'error');
      return;
    }

    const nextBaseWallet = draft.baseWallet.trim() || baseWallet.address?.trim() || '';
    if (!nextBaseWallet) {
      addToast('Connect a Base wallet or enter a Base wallet address first', 'error');
      return;
    }

    setBinding(true);
    try {
      const status = await api.bindLimitlessWallet({
        credentialId: selectedCredentialId,
        baseWallet: nextBaseWallet,
      });
      setCredentialStatus(status);
      await refreshCredentials();
      setDraft((current) => ({ ...current, baseWallet: status.base_wallet || nextBaseWallet }));
      addToast('Limitless wallet binding updated', 'success');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Wallet bind failed';
      addToast(message, 'error');
    } finally {
      setBinding(false);
    }
  }

  function loadMaskedCredential(entry: ExternalCredential) {
    setDraft({
      provider: entry.provider,
      label: entry.label,
      apiKey: '',
      apiSecret: '',
      apiPassphrase: '',
      baseWallet: String(entry.credentials.baseWallet ?? entry.credentials.base_wallet ?? ''),
      funder: String(entry.credentials.funder ?? ''),
      signatureType: String(entry.credentials.signatureType ?? entry.credentials.signature_type ?? '0'),
      defaultSignedOrder: entry.credentials.defaultSignedOrder
        ? JSON.stringify(entry.credentials.defaultSignedOrder, null, 2)
        : '',
    });
    setSelectedCredentialId(entry.id);
    addToast('Loaded credential metadata. Re-enter secrets to rotate it.', 'success');
  }

  return (
    <PageShell>
      <section className="mb-6 flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-text-primary">External Credentials</h1>
          <p className="mt-2 max-w-3xl text-sm text-text-secondary">
            {readOnly
              ? 'Live credential storage is disabled in this environment.'
              : 'Manage venue credentials for live external trading and external agents. These values are encrypted at rest.'}
          </p>
        </div>
        <div className="flex gap-2">
          <Link href="/agents" className="inline-flex h-10 items-center border border-border px-4 text-sm text-text-secondary transition-colors hover:border-border-hover hover:bg-bg-secondary hover:text-text-primary">
            Back to agents
          </Link>
        </div>
      </section>

      {readOnly ? (
        <div className="mb-6">
          <ReadOnlyNotice
            title="Credential management is currently unavailable"
            body="Live credential storage and updates are disabled in this environment."
            actionHref="/agents"
            actionLabel="Back to agents"
          />
        </div>
      ) : null}

      {readOnly ? null : (
        <>
          {!walletConnected ? (
            <Card className="mb-6">
              <h2 className="text-lg font-semibold text-text-primary">Connect and authenticate</h2>
              <p className="mt-2 text-sm text-text-secondary">
                Venue credentials are tied to your wallet account. Connect your wallet, then authenticate with SIWE before saving keys.
              </p>
            </Card>
          ) : !canManage ? (
            <Card className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
              <div>
                <h2 className="text-lg font-semibold text-text-primary">Session required</h2>
                <p className="mt-2 text-sm text-text-secondary">
                  Save, rotate, and delete flows require an authenticated session.
                </p>
              </div>
              <Button onClick={() => void login()} loading={isAuthenticating}>
                Authenticate wallet
              </Button>
            </Card>
          ) : null}

          <div className="grid gap-6 lg:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
            <Card>
              <div className="mb-5 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <div className="min-w-0 flex-1">
                  <h2 className="text-lg font-semibold text-text-primary">Credential vault</h2>
                  <p className="mt-1 text-sm text-text-secondary">
                    Keep one or more credentials per venue. External agents can reuse a stored <code>defaultSignedOrder</code>, and Limitless credentials also need a bound Base trading wallet.
                  </p>
                </div>
                <div className="grid w-full shrink-0 grid-cols-2 overflow-hidden border border-border sm:w-auto">
                  {(['limitless', 'polymarket'] as Provider[]).map((provider) => (
                    <button
                      key={provider}
                      type="button"
                      onClick={() => setDraft((current) => ({ ...current, provider }))}
                      className={
                        provider === draft.provider
                          ? 'h-10 min-w-[7.5rem] bg-accent/10 px-4 text-sm text-accent'
                          : 'h-10 min-w-[7.5rem] px-4 text-sm text-text-secondary'
                      }
                    >
                      {provider}
                    </button>
                  ))}
                </div>
              </div>

              {loading ? (
                <p className="text-sm text-text-secondary">Loading credentials…</p>
              ) : visibleCredentials.length === 0 ? (
                <p className="text-sm text-text-secondary">No {draft.provider} credentials saved yet.</p>
              ) : (
                <div className="space-y-3">
                  {visibleCredentials.map((entry) => (
                    <div key={entry.id} className="border border-border p-4">
                      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                        <div className="min-w-0">
                          <div className="text-sm font-medium uppercase tracking-[0.12em] text-text-primary">{entry.label}</div>
                          <div className="mt-2 space-y-1 text-xs text-text-secondary">
                            <div>ID: {entry.id}</div>
                            <div>Updated: {new Date(entry.updated_at).toLocaleString()}</div>
                            <div>Fields: {Object.keys(entry.credentials).join(', ') || 'none'}</div>
                            {entry.provider === 'limitless' && (entry.credentials.baseWallet || entry.credentials.base_wallet) ? (
                              <div>Base wallet: {String(entry.credentials.baseWallet ?? entry.credentials.base_wallet)}</div>
                            ) : null}
                            {entry.provider === 'polymarket' && entry.credentials.funder ? (
                              <div>Funder: {String(entry.credentials.funder)}</div>
                            ) : null}
                            {entry.provider === 'polymarket' && (entry.credentials.signatureType ?? entry.credentials.signature_type) !== undefined ? (
                              <div>
                                Signature type: {String(entry.credentials.signatureType ?? entry.credentials.signature_type)}
                              </div>
                            ) : null}
                          </div>
                        </div>
                        <div className="flex gap-2">
                          <Button
                            type="button"
                            variant="secondary"
                            size="sm"
                            disabled={!canManage}
                            onClick={() => loadMaskedCredential(entry)}
                          >
                            Rotate
                          </Button>
                          <Button
                            type="button"
                            variant="danger"
                            size="sm"
                            disabled={readOnly || !canManage}
                            onClick={() => void handleDelete(entry.id)}
                          >
                            Delete
                          </Button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </Card>

            <Card>
              <h2 className="text-lg font-semibold text-text-primary">Provider readiness</h2>
              <p className="mt-2 text-sm text-text-secondary">
                This reflects whether the currently selected provider credential is actually usable for live venue execution.
              </p>

              <div className="mt-5 space-y-3 border border-border p-4">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div>
                    <div className="text-sm font-medium text-text-primary">
                      {draft.provider} {credentialStatus?.ready ? 'ready' : 'not ready'}
                    </div>
                    <div className="mt-1 text-xs text-text-secondary">
                      Credential ID: {credentialStatus?.credential_id || 'none'}
                    </div>
                  </div>
                  {credentialStatus?.profile_status ? (
                    <span className="text-xs uppercase tracking-[0.12em] text-text-muted">
                      Profile {credentialStatus.profile_status.replaceAll('_', ' ')}
                    </span>
                  ) : null}
                </div>

                {credentialStatus?.base_wallet ? (
                  <div className="text-xs text-text-secondary">
                    Bound Base wallet: <span className="text-text-primary">{credentialStatus.base_wallet}</span>
                  </div>
                ) : null}

                <div className="space-y-2">
                  {(credentialStatus?.checks || []).map((check) => (
                    <div key={check.code} className="border border-border p-3 text-sm">
                      <div className="font-medium text-text-primary">
                        {check.ok ? 'OK' : 'Action required'} · {check.code.replaceAll('_', ' ')}
                      </div>
                      <div className="mt-1 text-text-secondary">{check.message}</div>
                    </div>
                  ))}
                </div>

                {draft.provider === 'limitless' ? (
                  <div className="flex flex-col gap-2 sm:flex-row">
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={!canManage || readOnly || !selectedCredentialId}
                      loading={binding}
                      onClick={() => void handleBindWallet()}
                    >
                      Bind trading wallet
                    </Button>
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={!baseWallet.address}
                      onClick={() =>
                        setDraft((current) => ({
                          ...current,
                          baseWallet: baseWallet.address || '',
                        }))
                      }
                    >
                      Use connected wallet
                    </Button>
                  </div>
                ) : null}
              </div>
            </Card>

            <Card>
              <h2 className="text-lg font-semibold text-text-primary">Save credential</h2>
              <p className="mt-2 text-sm text-text-secondary">
                Limitless requires an <code>apiKey</code> and a bound Base wallet. Polymarket
                credentials require the CLOB key set, funder wallet, and signature type. Browser
                wallet Polymarket accounts should use signature type <code>2</code>.
              </p>

              <form onSubmit={handleSave} className="mt-5 space-y-4">
                <Input
                  label="Label"
                  value={draft.label}
                  onChange={(event) => setDraft((current) => ({ ...current, label: event.target.value }))}
                  placeholder={`${draft.provider}-credential`}
                />
                <Input
                  label="API key"
                  value={draft.apiKey}
                  onChange={(event) => setDraft((current) => ({ ...current, apiKey: event.target.value }))}
                  placeholder="Required"
                />
                {draft.provider === 'limitless' ? (
                  <Input
                    label="Base trading wallet"
                    value={draft.baseWallet}
                    onChange={(event) =>
                      setDraft((current) => ({ ...current, baseWallet: event.target.value }))
                    }
                    placeholder="0x..."
                  />
                ) : null}
                {draft.provider === 'polymarket' ? (
                  <>
                    <div className="border border-border p-3 text-sm text-text-secondary">
                      Polymarket order submission uses the saved CLOB credential plus a wallet
                      signature from the connected account. Magic/email accounts are not supported by
                      the default signing flow here.
                    </div>
                    <Input
                      label="API secret"
                      value={draft.apiSecret}
                      onChange={(event) => setDraft((current) => ({ ...current, apiSecret: event.target.value }))}
                      placeholder="Required"
                    />
                    <Input
                      label="API passphrase"
                      value={draft.apiPassphrase}
                      onChange={(event) => setDraft((current) => ({ ...current, apiPassphrase: event.target.value }))}
                      placeholder="Required"
                    />
                    <Input
                      label="Funder wallet"
                      value={draft.funder}
                      onChange={(event) => setDraft((current) => ({ ...current, funder: event.target.value }))}
                      placeholder="0x..."
                    />
                    <div className="space-y-1.5">
                      <label className="block text-sm font-medium text-text-primary">Signature type</label>
                      <Select
                        value={draft.signatureType}
                        onChange={(event) =>
                          setDraft((current) => ({ ...current, signatureType: event.target.value }))
                        }
                        options={[
                          { value: '0', label: '0 · EOA' },
                          { value: '1', label: '1 · proxy' },
                          { value: '2', label: '2 · gnosis_safe' },
                        ]}
                      />
                    </div>
                  </>
                ) : null}
                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-text-primary" htmlFor="defaultSignedOrder">
                    Default signed order JSON
                  </label>
                  <textarea
                    id="defaultSignedOrder"
                    value={draft.defaultSignedOrder}
                    onChange={(event) => setDraft((current) => ({ ...current, defaultSignedOrder: event.target.value }))}
                    rows={10}
                    placeholder='Optional. Used by live external agents when no per-run signed order override is provided.'
                    className="w-full border border-border bg-bg-secondary px-3 py-2 text-sm text-text-primary placeholder:text-text-muted focus:outline-none focus:ring-2 focus:ring-accent/20"
                  />
                </div>
                <div className="flex gap-2">
                  <Button type="submit" loading={saving} disabled={!canManage || readOnly}>
                    Save credential
                  </Button>
                  <Button type="button" variant="secondary" onClick={() => setDraft((current) => ({ ...EMPTY_DRAFT, provider: current.provider }))}>
                    Reset
                  </Button>
                </div>
              </form>
            </Card>
          </div>
        </>
      )}
    </PageShell>
  );
}
