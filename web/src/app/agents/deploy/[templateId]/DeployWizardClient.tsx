'use client';

import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { useCallback, useMemo, useState } from 'react';
import { ArrowLeft, ArrowRight, Bot, Check, Shield, TrendingUp, Zap } from 'lucide-react';
import { PageShell } from '@/components/layout';
import { Button, Card, Badge, Input, Spinner, useToast } from '@/components/ui';
import { useAgentTemplate, useDeployManagedAgent } from '@/hooks';
import { formatCurrency } from '@/lib/utils';
import { cn } from '@/lib/utils';

type Step = 'review' | 'configure' | 'fund' | 'deploy';
const STEPS: Step[] = ['review', 'configure', 'fund', 'deploy'];

const STEP_LABELS: Record<Step, string> = {
  review: 'Review template',
  configure: 'Configure',
  fund: 'Fund',
  deploy: 'Deploy',
};

function StepIndicator({ current, steps }: { current: Step; steps: Step[] }) {
  const currentIdx = steps.indexOf(current);

  return (
    <div className="flex items-center gap-2">
      {steps.map((step, i) => {
        const isCompleted = i < currentIdx;
        const isCurrent = i === currentIdx;

        return (
          <div key={step} className="flex items-center gap-2">
            {i > 0 && (
              <div
                className={cn(
                  'h-px w-6 sm:w-10',
                  isCompleted ? 'bg-accent' : 'bg-border',
                )}
              />
            )}
            <div className="flex items-center gap-1.5">
              <div
                className={cn(
                  'flex h-6 w-6 items-center justify-center text-[0.65rem] font-semibold border',
                  isCompleted && 'bg-accent border-accent text-text-inverse',
                  isCurrent && 'border-accent text-accent',
                  !isCompleted && !isCurrent && 'border-border text-text-muted',
                )}
              >
                {isCompleted ? <Check className="h-3 w-3" /> : i + 1}
              </div>
              <span
                className={cn(
                  'hidden sm:inline text-xs',
                  isCurrent ? 'text-text-primary font-medium' : 'text-text-muted',
                )}
              >
                {STEP_LABELS[step]}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function DeployWizardClient({ templateId }: { templateId: string }) {
  const router = useRouter();
  const { addToast } = useToast();
  const { data: template, isLoading } = useAgentTemplate(templateId);
  const deployMutation = useDeployManagedAgent();

  const [step, setStep] = useState<Step>('review');
  const [agentName, setAgentName] = useState('');
  const [seedUsdc, setSeedUsdc] = useState('');
  const [customParams, setCustomParams] = useState<Record<string, string>>({});
  const [fundConfirmed, setFundConfirmed] = useState(false);

  const defaultParamKeys = useMemo(() => {
    if (!template?.defaultParams) return [];
    return Object.keys(template.defaultParams);
  }, [template]);

  const canProceed = useCallback(() => {
    if (!template) return false;
    if (step === 'review') return true;
    if (step === 'configure') {
      const name = agentName.trim();
      const seed = parseFloat(seedUsdc);
      return name.length > 0 && name.length <= 128 && seed >= template.minSeedUsdc;
    }
    if (step === 'fund') return fundConfirmed;
    return false;
  }, [step, template, agentName, seedUsdc, fundConfirmed]);

  const goNext = useCallback(() => {
    const idx = STEPS.indexOf(step);
    if (idx < STEPS.length - 1) setStep(STEPS[idx + 1]);
  }, [step]);

  const goBack = useCallback(() => {
    const idx = STEPS.indexOf(step);
    if (idx > 0) setStep(STEPS[idx - 1]);
  }, [step]);

  const handleDeploy = useCallback(async () => {
    if (!template) return;

    const mergedParams: Record<string, unknown> = {};
    for (const key of defaultParamKeys) {
      const userVal = customParams[key]?.trim();
      if (userVal !== undefined && userVal !== '') {
        const num = Number(userVal);
        mergedParams[key] = Number.isFinite(num) ? num : userVal;
      }
    }

    try {
      await deployMutation.mutateAsync({
        templateId: template.id,
        name: agentName.trim(),
        seedUsdc: parseFloat(seedUsdc),
        params: Object.keys(mergedParams).length > 0 ? mergedParams : undefined,
      });
      addToast('Agent deployed successfully', 'success');
      router.push('/agents');
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Deployment failed';
      addToast(msg, 'error');
    }
  }, [template, agentName, seedUsdc, customParams, defaultParamKeys, deployMutation, router, addToast]);

  if (isLoading) {
    return (
      <PageShell>
        <div className="flex items-center justify-center py-24">
          <Spinner />
        </div>
      </PageShell>
    );
  }

  if (!template) {
    return (
      <PageShell>
        <div className="flex flex-col items-center justify-center gap-4 py-24">
          <p className="text-sm text-text-muted">Template not found.</p>
          <Link href="/agents/templates">
            <Button variant="secondary" size="sm">
              Browse templates
            </Button>
          </Link>
        </div>
      </PageShell>
    );
  }

  const riskIcons: Record<string, React.ReactNode> = {
    low: <Shield className="h-3.5 w-3.5" />,
    medium: <TrendingUp className="h-3.5 w-3.5" />,
    high: <Zap className="h-3.5 w-3.5" />,
  };

  return (
    <PageShell>
      <div className="mx-auto max-w-2xl space-y-6">
        <div className="flex items-center gap-3">
          <Link href="/agents/templates">
            <Button variant="ghost" size="sm" className="px-2">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <h1 className="text-lg font-bold text-text-primary">Deploy agent</h1>
        </div>

        <StepIndicator current={step} steps={STEPS} />

        {step === 'review' && (
          <Card className="space-y-4">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center bg-bg-tertiary border border-border">
                <Bot className="h-5 w-5 text-accent" />
              </div>
              <div>
                <h2 className="text-sm font-semibold text-text-primary">{template.name}</h2>
                <p className="text-xs text-text-muted">{template.strategy}</p>
              </div>
            </div>

            {template.description && (
              <p className="text-xs text-text-secondary leading-relaxed">
                {template.description}
              </p>
            )}

            <div className="grid grid-cols-3 gap-4 text-xs">
              <div>
                <span className="text-text-muted">Category</span>
                <p className="text-text-primary font-medium capitalize">{template.category}</p>
              </div>
              <div>
                <span className="text-text-muted">Risk tier</span>
                <div className="flex items-center gap-1 text-text-primary font-medium capitalize">
                  {riskIcons[template.riskTier]}
                  {template.riskTier}
                </div>
              </div>
              <div>
                <span className="text-text-muted">Min seed</span>
                <p className="text-text-primary font-medium">
                  {formatCurrency(template.minSeedUsdc)}
                </p>
              </div>
            </div>

            {defaultParamKeys.length > 0 && (
              <div>
                <p className="text-xs text-text-muted mb-2">Default parameters</p>
                <div className="grid grid-cols-2 gap-2">
                  {defaultParamKeys.map((key) => (
                    <div key={key} className="text-xs">
                      <span className="text-text-muted">{key}</span>
                      <p className="text-text-primary font-mono">
                        {String(template.defaultParams[key])}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </Card>
        )}

        {step === 'configure' && (
          <Card className="space-y-5">
            <h2 className="text-sm font-semibold text-text-primary">Configuration</h2>

            <Input
              label="Agent name"
              placeholder="My momentum agent"
              value={agentName}
              onChange={(e) => setAgentName(e.target.value)}
              hint="1-128 characters"
            />

            <Input
              label="Seed amount (USDC)"
              type="number"
              placeholder={String(template.minSeedUsdc)}
              value={seedUsdc}
              onChange={(e) => setSeedUsdc(e.target.value)}
              hint={`Minimum ${formatCurrency(template.minSeedUsdc)}`}
              error={
                seedUsdc && parseFloat(seedUsdc) < template.minSeedUsdc
                  ? `Must be at least ${formatCurrency(template.minSeedUsdc)}`
                  : undefined
              }
            />

            {defaultParamKeys.length > 0 && (
              <div className="space-y-3">
                <p className="text-xs font-medium text-text-primary">
                  Strategy parameters{' '}
                  <span className="text-text-muted font-normal">(optional overrides)</span>
                </p>
                {defaultParamKeys.map((key) => (
                  <Input
                    key={key}
                    label={key}
                    placeholder={String(template.defaultParams[key])}
                    value={customParams[key] ?? ''}
                    onChange={(e) =>
                      setCustomParams((prev) => ({ ...prev, [key]: e.target.value }))
                    }
                    hint={`Default: ${String(template.defaultParams[key])}`}
                  />
                ))}
              </div>
            )}
          </Card>
        )}

        {step === 'fund' && (
          <Card className="space-y-5">
            <h2 className="text-sm font-semibold text-text-primary">Fund your agent</h2>
            <p className="text-xs text-text-secondary leading-relaxed">
              Ensure your account balance has at least{' '}
              <span className="font-semibold text-text-primary">
                {formatCurrency(parseFloat(seedUsdc) || template.minSeedUsdc)}
              </span>{' '}
              USDC available. This amount will be allocated to the agent upon deployment.
            </p>

            <div className="border border-border bg-bg-secondary p-4 space-y-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-text-muted">Template</span>
                <span className="text-text-primary font-medium">{template.name}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-text-muted">Agent name</span>
                <span className="text-text-primary font-medium">{agentName}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-text-muted">Seed amount</span>
                <span className="text-text-primary font-semibold">
                  {formatCurrency(parseFloat(seedUsdc) || 0)}
                </span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-text-muted">Risk tier</span>
                <Badge
                  variant={
                    template.riskTier === 'low'
                      ? 'success'
                      : template.riskTier === 'medium'
                        ? 'warning'
                        : 'danger'
                  }
                  className="capitalize"
                >
                  {template.riskTier}
                </Badge>
              </div>
            </div>

            <label className="flex items-start gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={fundConfirmed}
                onChange={(e) => setFundConfirmed(e.target.checked)}
                className="mt-0.5 accent-accent"
              />
              <span className="text-xs text-text-secondary">
                I confirm that I have sufficient USDC balance and understand the risks associated
                with a <span className="capitalize">{template.riskTier}</span>-risk strategy.
              </span>
            </label>
          </Card>
        )}

        {step === 'deploy' && (
          <Card className="space-y-5">
            <h2 className="text-sm font-semibold text-text-primary">Ready to deploy</h2>
            <p className="text-xs text-text-secondary leading-relaxed">
              Your managed agent will start executing the{' '}
              <span className="font-medium text-text-primary">{template.strategy}</span> strategy
              immediately after deployment.
            </p>

            <Button
              variant="primary"
              size="lg"
              className="w-full"
              loading={deployMutation.isPending}
              onClick={handleDeploy}
            >
              {deployMutation.isPending ? 'Deploying...' : 'Deploy agent'}
            </Button>

            {deployMutation.isError && (
              <p className="text-xs text-text-muted text-center">
                {deployMutation.error?.message ?? 'Something went wrong'}
              </p>
            )}
          </Card>
        )}

        <div className="flex items-center justify-between">
          {step !== 'review' ? (
            <Button variant="ghost" size="sm" onClick={goBack}>
              <ArrowLeft className="h-3.5 w-3.5 mr-1" />
              Back
            </Button>
          ) : (
            <div />
          )}

          {step !== 'deploy' && (
            <Button
              variant="primary"
              size="sm"
              disabled={!canProceed()}
              onClick={goNext}
            >
              Next
              <ArrowRight className="h-3.5 w-3.5 ml-1" />
            </Button>
          )}
        </div>
      </div>
    </PageShell>
  );
}
