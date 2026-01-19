# Frontend CI/CD Pipeline

Enterprise-level continuous integration and deployment for the Eventify frontend application.

## Pipeline Overview

### CI Pipeline (`.github/workflows/ci.yml`)

The CI pipeline runs on every pull request and push to main/develop branches:

| Stage | Job | Description |
|-------|-----|-------------|
| 1 | `lint-and-typecheck` | ESLint, Prettier, TypeScript type checking |
| 2 | `test` | Unit tests with Vitest + Playwright browser tests |
| 3 | `security-audit` | npm audit, SonarQube security scanning |
| 4 | `build` | Production build with artifact upload |
| 5 | `integration-test` | End-to-end tests on built preview |

### CD Pipeline (`.github/workflows/cd.yml`)

The CD pipeline deploys to staging and production environments:

| Environment | Trigger | Strategy |
|------------|---------|----------|
| Staging | Push to `develop` or manual trigger | Rolling update |
| Production | Push to `main` or manual trigger | Blue/Green with rollback |

## Getting Started

### Prerequisites

- Node.js 20+
- npm or bun package manager
- Docker (for containerized builds)

### Local Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Run tests
npm run test          # Unit tests
npm run test:e2e      # E2E tests
npm run test:ci       # CI-compatible test run

# Type checking
npm run typecheck

# Linting
npm run lint
npm run lint:fix

# Formatting
npm run format
npm run format:check

# Build for production
npm run build

# Preview production build
npm run preview
```

## CI/CD Configuration

### GitHub Actions Secrets

Configure these secrets in GitHub repository settings:

| Secret | Description |
|--------|-------------|
| `SONAR_TOKEN` | SonarQube authentication token |
| `SLACK_WEBHOOK_URL` | Slack notifications webhook |
| `API_BASE_URL` | Backend API URL (staging/production) |
| `GHCR_TOKEN` | GitHub Container Registry token |

### Required GitHub Permissions

- `contents: read` - Checkout code
- `packages: write` - Push Docker images
- `id-token: write` - OIDC for cloud deployments
- `pull-requests: write` - Create release PRs

## Kubernetes Deployment

### Staging (`k8s/staging/`)

- 2 replicas
- ClusterIP service
- NGINX Ingress with Let's Encrypt
- PodDisruptionBudget for availability

### Production (`k8s/production/`)

- 3 replicas minimum
- HorizontalPodAutoscaler (3-10 replicas)
- TLS with cert-manager
- Security context (non-root, read-only filesystem)
- PodDisruptionBudget for zero-downtime deployments

### Required Secrets

```bash
kubectl create secret generic frontend-secrets \
  --from-literal=api-base-url=https://api.eventify.app \
  -n eventify-production

kubectl create secret docker-registry ghcr-secret \
  --docker-server=ghcr.io \
  --docker-username=<github-username> \
  --docker-password=<ghcr-token> \
  -n eventify-production
```

## Docker Build

### Multi-stage Dockerfile

```dockerfile
# Builder stage
FROM node:20-alpine AS builder
RUN npm ci && npm run build

# Production stage  
FROM nginx:alpine AS production
COPY --from=builder /dist /usr/share/nginx/html
```

### Build & Push

```bash
# Build image
docker build -t ghcr.io/eventify/frontend:latest .

# Push to registry
docker push ghcr.io/eventify/frontend:latest
```

## Testing Strategy

### Unit Tests (Vitest)

```bash
npm run test:unit
```

Located in `src/**/*.test.tsx` with coverage reporting.

### E2E Tests (Playwright)

```bash
npm run test:e2e
```

Located in `tests/**/*.spec.ts` running across:
- Desktop: Chrome, Firefox, Safari
- Mobile: iPhone 12, Pixel 5

### Integration Tests

Run against preview server after successful build:
```bash
npm run build && npm run preview
```

## Pipeline Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    PUSH / PULL REQUEST                      │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     LINT & TYPECHECK                        │
│  • ESLint (code quality)                                    │
│  • Prettier (code formatting)                               │
│  • TypeScript (type safety)                                 │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                       UNIT TESTS                            │
│  • Vitest with jsdom                                        │
│  • Coverage report (Codecov)                                │
│  • Playwright browser tests                                 │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    SECURITY AUDIT                           │
│  • npm audit                                                │
│  • SonarQube (code quality/security)                        │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                       BUILD                                 │
│  • Production build                                         │
│  • Docker image creation                                    │
│  • Artifact upload                                          │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                   INTEGRATION TESTS                         │
│  • Preview server start                                     │
│  • Playwright E2E tests                                     │
│  • Artifact upload                                          │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
              ┌──────────┴──────────┐
              ▼                     ▼
    ┌─────────────────┐    ┌─────────────────┐
    │    STAGING      │    │   PRODUCTION    │
    │  (auto-deploy)  │    │   (manual or    │
    │                 │    │   main branch)  │
    └─────────────────┘    └─────────────────┘
```

## Rollback Procedure

The pipeline automatically rolls back on:

1. **Health check failure** - Detected during deployment
2. **Test failure** - E2E tests fail
3. **Manual trigger** - Via GitHub Actions UI

Manual rollback:
```bash
kubectl rollout undo deployment/frontend -n eventify-production
kubectl rollout status deployment/frontend -n eventify-production --timeout=300s
```

## Monitoring

### Health Endpoints

- `/health` - Liveness probe
- `/metrics` - Prometheus metrics (when configured)

### Alerts

Configured Slack notifications for:
- Deployment success/failure
- Rollback triggers
- Security vulnerabilities

## Contributing

### Adding New Tests

**Unit test:** Add `*.test.tsx` to `src/components/`
**E2E test:** Add `*.spec.ts` to `tests/`

### Pipeline Modifications

1. Update `.github/workflows/ci.yml` for CI changes
2. Update `.github/workflows/cd.yml` for CD changes
3. Update `k8s/` manifests for deployment changes
