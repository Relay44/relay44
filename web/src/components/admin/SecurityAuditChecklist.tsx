'use client';

import { useState } from 'react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { cn } from '@/lib/utils';

interface ChecklistItem {
  id: string;
  title: string;
  description: string;
  status: 'complete' | 'in_progress' | 'pending' | 'na';
  priority: 'critical' | 'high' | 'medium' | 'low';
  category: string;
  notes?: string;
}

interface ChecklistCategory {
  name: string;
  items: ChecklistItem[];
}

const CHECKLIST: ChecklistCategory[] = [
  {
    name: 'Smart Contract Security',
    items: [
      {
        id: 'sc-1',
        title: 'Reentrancy Analysis',
        description: 'Review all external contract calls for reentrancy vulnerabilities',
        status: 'complete',
        priority: 'critical',
        category: 'Smart Contract Security',
        notes: 'External calls are guarded and state updates are ordered around lock/unlock flows.',
      },
      {
        id: 'sc-2',
        title: 'Arithmetic Overflow Protection',
        description: 'Verify all arithmetic operations enforce safe bounds and fixed-point precision',
        status: 'complete',
        priority: 'critical',
        category: 'Smart Contract Security',
        notes: 'Core math paths are covered by unit tests and revert on invalid ranges.',
      },
      {
        id: 'sc-3',
        title: 'Account Ownership Verification',
        description: 'Ensure all privileged methods enforce role and ownership checks',
        status: 'complete',
        priority: 'critical',
        category: 'Smart Contract Security',
      },
      {
        id: 'sc-4',
        title: 'Access Control Hardening',
        description: 'Review admin/operator/resolver permissions and timelock boundaries',
        status: 'complete',
        priority: 'high',
        category: 'Smart Contract Security',
      },
      {
        id: 'sc-5',
        title: 'Input Validation',
        description: 'All user inputs validated for bounds and format',
        status: 'in_progress',
        priority: 'high',
        category: 'Smart Contract Security',
        notes: 'Price bounds checked. Need to add MAX_ORDER_QUANTITY.',
      },
      {
        id: 'sc-6',
        title: 'Oracle Staleness Check',
        description: 'Implement price staleness validation for oracle data',
        status: 'pending',
        priority: 'medium',
        category: 'Smart Contract Security',
      },
      {
        id: 'sc-7',
        title: 'Fuzz Testing',
        description: 'Foundry fuzz/invariant stress tests for market and orderbook logic',
        status: 'complete',
        priority: 'high',
        category: 'Smart Contract Security',
        notes: 'Run via scripts/fuzz-campaign.sh against evm/ contracts.',
      },
    ],
  },
  {
    name: 'Backend API Security',
    items: [
      {
        id: 'api-1',
        title: 'JWT Implementation',
        description: 'Secure JWT generation, validation, and rotation',
        status: 'complete',
        priority: 'critical',
        category: 'Backend API Security',
        notes: 'Key rotation mechanism implemented with kid support.',
      },
      {
        id: 'api-2',
        title: 'Rate Limiting',
        description: 'Per-endpoint rate limits on all mutating operations',
        status: 'complete',
        priority: 'critical',
        category: 'Backend API Security',
        notes: 'Orders: 10/min, Market creation: 1/hr, Claims: 5/min.',
      },
      {
        id: 'api-3',
        title: 'Security Headers',
        description: 'X-Content-Type-Options, X-Frame-Options, CSP, etc.',
        status: 'complete',
        priority: 'critical',
        category: 'Backend API Security',
      },
      {
        id: 'api-4',
        title: 'SQL Injection Prevention',
        description: 'Parameterized queries for all database operations',
        status: 'complete',
        priority: 'critical',
        category: 'Backend API Security',
      },
      {
        id: 'api-5',
        title: 'Input Validation',
        description: 'Request body size limits and schema validation',
        status: 'complete',
        priority: 'high',
        category: 'Backend API Security',
        notes: 'JSON body limit: 4KB.',
      },
      {
        id: 'api-6',
        title: 'WebSocket Authentication',
        description: 'Require authentication for WebSocket connections',
        status: 'pending',
        priority: 'high',
        category: 'Backend API Security',
      },
      {
        id: 'api-7',
        title: 'Request Tracing',
        description: 'X-Request-ID for all requests for audit trail',
        status: 'complete',
        priority: 'medium',
        category: 'Backend API Security',
      },
      {
        id: 'api-8',
        title: 'Idempotency Keys',
        description: 'Prevent duplicate order placement on network issues',
        status: 'complete',
        priority: 'medium',
        category: 'Backend API Security',
      },
    ],
  },
  {
    name: 'Frontend Security',
    items: [
      {
        id: 'fe-1',
        title: 'Token Storage',
        description: 'Access tokens in memory, refresh tokens in httpOnly cookies',
        status: 'complete',
        priority: 'critical',
        category: 'Frontend Security',
      },
      {
        id: 'fe-2',
        title: 'XSS Prevention',
        description: 'React auto-escaping, no dangerouslySetInnerHTML',
        status: 'complete',
        priority: 'critical',
        category: 'Frontend Security',
      },
      {
        id: 'fe-3',
        title: 'CSRF Protection',
        description: 'SameSite cookies for session management',
        status: 'complete',
        priority: 'critical',
        category: 'Frontend Security',
      },
      {
        id: 'fe-4',
        title: 'Error Boundaries',
        description: 'Graceful error handling without exposing internals',
        status: 'complete',
        priority: 'high',
        category: 'Frontend Security',
      },
      {
        id: 'fe-5',
        title: 'Dependency Audit',
        description: 'npm audit with no high/critical vulnerabilities',
        status: 'pending',
        priority: 'high',
        category: 'Frontend Security',
      },
    ],
  },
  {
    name: 'Infrastructure Security',
    items: [
      {
        id: 'infra-1',
        title: 'Secrets Management',
        description: 'External Secrets Operator for production secrets',
        status: 'complete',
        priority: 'critical',
        category: 'Infrastructure Security',
      },
      {
        id: 'infra-2',
        title: 'Database Encryption',
        description: 'Encryption at rest and in transit',
        status: 'pending',
        priority: 'critical',
        category: 'Infrastructure Security',
      },
      {
        id: 'infra-3',
        title: 'Network Segmentation',
        description: 'VPC configuration with proper security groups',
        status: 'pending',
        priority: 'high',
        category: 'Infrastructure Security',
      },
      {
        id: 'infra-4',
        title: 'Backup Automation',
        description: 'Automated database backups with tested restore',
        status: 'complete',
        priority: 'high',
        category: 'Infrastructure Security',
      },
      {
        id: 'infra-5',
        title: 'Geo-Blocking',
        description: 'Block restricted jurisdictions at CDN level',
        status: 'complete',
        priority: 'high',
        category: 'Infrastructure Security',
      },
      {
        id: 'infra-6',
        title: 'WAF Configuration',
        description: 'Web Application Firewall rules',
        status: 'pending',
        priority: 'medium',
        category: 'Infrastructure Security',
      },
    ],
  },
  {
    name: 'Operational Security',
    items: [
      {
        id: 'ops-1',
        title: 'Incident Response Plan',
        description: 'Documented procedures for security incidents',
        status: 'complete',
        priority: 'critical',
        category: 'Operational Security',
        notes: 'docs/runbooks/INCIDENT_RESPONSE.md',
      },
      {
        id: 'ops-2',
        title: 'Disaster Recovery Plan',
        description: 'Documented procedures for system recovery',
        status: 'complete',
        priority: 'critical',
        category: 'Operational Security',
        notes: 'docs/runbooks/DISASTER_RECOVERY.md',
      },
      {
        id: 'ops-3',
        title: 'Alerting Configuration',
        description: 'Prometheus/AlertManager rules for anomaly detection',
        status: 'complete',
        priority: 'high',
        category: 'Operational Security',
      },
      {
        id: 'ops-4',
        title: 'Access Control',
        description: 'Role-based access for admin functions',
        status: 'pending',
        priority: 'high',
        category: 'Operational Security',
      },
      {
        id: 'ops-5',
        title: 'Audit Logging',
        description: 'Comprehensive logging of security-relevant events',
        status: 'in_progress',
        priority: 'high',
        category: 'Operational Security',
      },
    ],
  },
];

