import { Card } from '@/components/ui';

export interface Endpoint {
  method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE';
  path: string;
  description: string;
  auth?: boolean;
  example?: string;
}

interface EndpointGroupProps {
  title: string;
  description?: string;
  endpoints: Endpoint[];
}

const methodColor: Record<string, string> = {
  GET: 'text-emerald-500 border-emerald-500/30',
  POST: 'text-blue-500 border-blue-500/30',
  PATCH: 'text-amber-500 border-amber-500/30',
  PUT: 'text-amber-500 border-amber-500/30',
  DELETE: 'text-red-500 border-red-500/30',
};

export function EndpointGroup({ title, description, endpoints }: EndpointGroupProps) {
  return (
    <Card className="p-6">
      <h2 className="text-lg font-semibold text-text-primary">{title}</h2>
      {description && (
        <p className="mt-2 text-sm leading-6 text-text-secondary">{description}</p>
      )}
      <div className="mt-4 overflow-hidden border border-border">
        {endpoints.map((ep) => (
          <div
            key={`${ep.method}-${ep.path}`}
            className="grid gap-2 border-b border-border px-4 py-3 last:border-b-0 md:grid-cols-[5.5rem_minmax(0,1fr)]"
          >
            <span
              className={`inline-flex h-8 w-fit items-center border px-3 text-[0.75rem] font-medium uppercase tracking-[0.14em] ${methodColor[ep.method] || 'text-text-primary border-border'}`}
            >
              {ep.method}
            </span>
            <div className="min-w-0">
              <code className="block overflow-x-auto text-sm text-text-primary">{ep.path}</code>
              <p className="mt-1 text-xs uppercase tracking-[0.12em] text-text-muted">
                {ep.description}
              </p>
              {ep.auth && (
                <span className="mt-1 inline-block text-[0.65rem] uppercase tracking-widest text-amber-500">
                  Requires auth
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}
