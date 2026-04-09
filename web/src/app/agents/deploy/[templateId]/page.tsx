import { buildPageMetadata } from '@/lib/seo';
import { DeployWizardClient } from './DeployWizardClient';

export const metadata = buildPageMetadata({
  title: 'Deploy agent',
  description: 'Configure and deploy a managed trading agent from a template.',
  path: '/agents/deploy',
  noIndex: true,
});

interface DeployPageProps {
  params: Promise<{ templateId: string }>;
}

export default async function DeployAgentPage({ params }: DeployPageProps) {
  const { templateId } = await params;
  return <DeployWizardClient templateId={templateId} />;
}
