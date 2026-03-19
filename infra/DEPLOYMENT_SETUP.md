# Deployment Setup

Configuration guide for GitHub Actions deployment workflows.

## GitHub Environments

Create these environments in your GitHub repository settings:

### staging

- No protection rules required
- Secrets:
  - `STAGING_KUBECONFIG` - Base64-encoded kubeconfig for staging cluster
  - `DATABASE_URL_STAGING` - PostgreSQL connection string

### staging-rollback

- Same secrets as `staging`
- Used for rollback operations

### production

- **Required reviewers**: Add at least one team member
- **Wait timer**: 5 minutes (optional, for final review)
- Secrets:
  - `PRODUCTION_KUBECONFIG` - Base64-encoded kubeconfig for production cluster
  - `DATABASE_URL_PRODUCTION` - PostgreSQL connection string

### production-rollback

- **Required reviewers**: Add at least one team member (emergency override possible)
- Same secrets as `production`

### production-programs

- **Required reviewers**: Add multiple team members for Solana program deployments
- Secrets:
  - `MAINNET_RPC_URL` - Solana mainnet RPC endpoint
  - `MAINNET_DEPLOYER_KEYPAIR` - JSON array of deployer keypair bytes

## Required Secrets

### Kubernetes Secrets

Generate base64-encoded kubeconfig:

```bash
# Staging
cat ~/.kube/staging-config | base64 -w 0 > staging-kubeconfig.b64

# Production
cat ~/.kube/production-config | base64 -w 0 > production-kubeconfig.b64
```

### Solana Secrets

1. Create deployer keypair (if not exists):
```bash
cast wallet new --json > base-mainnet-deployer.json
```

2. Fund the deployer with at least 5 SOL for program deployment

3. Add the keypair JSON content as `MAINNET_DEPLOYER_KEYPAIR`:
```bash
cat mainnet-deployer.json
# Copy the JSON array, e.g., [1,2,3,...]
```

### Database URLs

Format: `postgres://user:password@host:5432/database?sslmode=require`

## Workflow Triggers

### Automatic Deployments

- **Push to main**: Runs CI, deploys to staging if tests pass
- **Tag push (v*)**: Creates release, deploys to staging, requests production approval

### Manual Deployments

Use `workflow_dispatch` for:

1. **Deploy Mainnet** (`deploy-mainnet.yml`)
   - Select environment: staging or production
   - Select target: api-only, programs-only, or full
   - Optional: specific image tag or program version
   - Optional: dry-run mode

2. **Rollback** (`rollback.yml`)
   - Select environment
   - Select rollback type: previous, specific-revision, or specific-image
   - Provide reason for audit trail

## Production Deployment Flow

```
1. Create release tag
   git tag v1.2.3
   git push origin v1.2.3

2. CI builds and tests
   - Unit tests
   - Integration tests
   - Security audit
   - Program build

3. Staging deployment (automatic)
   - Database migrations
   - Rolling deployment
   - Health check

4. Production approval
   - Reviewer notified
   - Manual approval required

5. Production deployment
   - Database migrations
   - Rolling deployment
   - Health check
   - Auto-rollback on failure
```

## Monitoring Deployments

### GitHub Actions

Check workflow status at:
```
https://github.com/OWNER/REPO/actions
```

### Kubernetes

```bash
# Check deployment status
kubectl get deployments -n relay44-production

# Check pod status
kubectl get pods -n relay44-production

# View logs
kubectl logs -f deployment/relay44-api-blue -n relay44-production

# View rollout history
kubectl rollout history deployment/relay44-api-blue -n relay44-production
```

### Solana Program

```bash
# Verify deployment
cast code <CONTRACT_ADDRESS> --rpc-url https://mainnet.base.org
```

## Emergency Procedures

### API Rollback

1. Go to Actions > Rollback
2. Select environment: production
3. Select rollback type: previous
4. Enter reason
5. Run workflow

Or via kubectl:
```bash
kubectl rollout undo deployment/relay44-api-blue -n relay44-production
```

### Program Upgrade Failure

Solana programs cannot be rolled back. Options:

1. Deploy a fixed version as an upgrade
2. If authority allows, close and redeploy (loses state)
3. Transfer upgrade authority to multisig for future safety

