# Zero To Production in Rust (`zero2prod`)

Production-grade newsletter service built with Rust, Axum, SQLx, and Tokio, deployed to Google Cloud Run with Supabase PostgreSQL and Google Cloud Secret Manager.

---

## Infrastructure Overview

- **Compute**: Google Cloud Run (Region: `asia-southeast1` - Singapore)
- **Container Registry**: Google Artifact Registry (`asia-southeast1-docker.pkg.dev/zero2prod-1790050760/zero2prod-repo`)
- **Database**: Supabase PostgreSQL (AWS Singapore via IPv4 Session Pooler)
- **Secret Management**: Google Cloud Secret Manager
- **Packaging**: 4-stage Alpine Docker build with `cargo-chef` (~26 MB image)

---

## Prerequisites & Initial Setup

1. **Google Cloud SDK (`gcloud`)**:
   Ensure `gcloud` is logged in and configured to the active project:
   ```bash
   gcloud auth login
   gcloud config set project zero2prod-1790050760
   ```

2. **Docker Artifact Registry Authentication**:
   Authenticate Docker to push to Google Artifact Registry:
   ```bash
   gcloud auth configure-docker asia-southeast1-docker.pkg.dev
   ```

3. **Environment File (`.env`)**:
   Create a `.env` file in the project root. This file stores local credentials and secrets used by Makefile tasks:
   ```env
   # Database Password for Supabase
   SUPABASE_DB_PASSWORD=your-database-password-here
   ```
   *(Note: `.env` is listed in `.gitignore` and must never be committed to git).*

---

## Makefile Command Reference

Run `make help` at any time to list all available targets:

```bash
make help
```

### 1. Deployment & Operations

| Command | Description |
| :--- | :--- |
| `make build` | Builds the multi-stage Alpine Docker image locally using `Dockerfile`. |
| `make push` | Pushes the Docker image to Google Artifact Registry. |
| `make deploy` | Applies `service.yaml` to Cloud Run (updates revisions, env vars, secret mounts). |
| `make release` | **Full pipeline**: Runs `make build` → `make push` → `make deploy`. |
| `make logs` | Tails live production logs from Cloud Run in your terminal. |
| `make url` | Prints the live Cloud Run URL and executes a `/health_check` request. |

### 2. Secret Management (Google Secret Manager)

All secret commands automatically grant the Cloud Run default service account (`737112983679-compute@developer.gserviceaccount.com`) the `roles/secretmanager.secretAccessor` role.

#### Secret Value Resolution Order
When running `add-secret` or `update-secret`, the value is resolved in this order:
1. **Explicit argument**: `VALUE="my-secret"`
2. **Auto-detected from `.env`**: Matches either exact name (e.g. `supabase-db-password=...`) or UPPER_CASE_SNAKE (e.g. `SUPABASE_DB_PASSWORD=...`).
3. **Interactive hidden prompt**: If omitted from both command line and `.env`, you will be prompted securely via terminal (`read -sp`).

| Command | Description | Example |
| :--- | :--- | :--- |
| `make add-secret` | Creates a new secret in Secret Manager and grants Cloud Run read access. | `make add-secret NAME=supabase-db-password` |
| `make update-secret` | Adds a new version to an existing secret (used for secret rotation). | `make update-secret NAME=supabase-db-password` |
| `make remove-secret` | Deletes a secret from Secret Manager. | `make remove-secret NAME=old-unused-secret` |
| `make list-secrets` | Lists all secrets currently in Secret Manager. | `make list-secrets` |

### 3. Database & Migrations

| Command | Description |
| :--- | :--- |
| `make init-db` | Starts a fresh local PostgreSQL container via `scripts/init_db.sh` and runs migrations. |
| `make migrate` | Applies all pending migrations in `migrations/` to the local PostgreSQL database. |
| `make migrate-prod` | Applies pending migrations to the production Supabase database (reads password from `.env`). |
| `make prepare` | Updates the `.sqlx` offline query cache (`cargo sqlx prepare -- --all-targets`) needed for Docker builds. |

### 4. Local Development & Testing

| Command | Description |
| :--- | :--- |
| `make test` | Runs the integration test suite against your local Docker PostgreSQL container. |

---

## Common Workflows

### Workflow 1: Releasing a New Application Version

When you make changes to application code:

```bash
# 1. Verify tests pass locally
make test

# 2. Build, push, and deploy to Cloud Run
make release

# 3. Verify health and stream logs
make url
make logs
```

---

### Workflow 2: Adding a New Secret to Cloud Run

To introduce a new secret (e.g., `stripe-api-key`):

1. **Add it to your `.env`**:
   ```env
   STRIPE_API_KEY=sk_live_123456789
   ```

2. **Upload it to Secret Manager using Make**:
   ```bash
   make add-secret NAME=stripe-api-key
   ```
   *(Make reads `STRIPE_API_KEY` from `.env`, creates the secret, and grants Cloud Run access).*

3. **Mount the secret in `service.yaml`**:
   Under `spec.template.spec.containers[0].env`:
   ```yaml
   - name: APP_STRIPE__API_KEY
     valueFrom:
       secretKeyRef:
         name: stripe-api-key
         key: latest
   ```

4. **Deploy the configuration change**:
   ```bash
   make deploy
   ```

---

### Workflow 3: Rotating an Existing Secret

When a database password or API token changes:

1. Update the value in `.env`:
   ```env
   SUPABASE_DB_PASSWORD=new_secure_password_here
   ```

2. Push the new version to Secret Manager:
   ```bash
   make update-secret NAME=supabase-db-password
   ```

3. Trigger a redeploy so Cloud Run mounts the new `latest` version:
   ```bash
   make deploy
   ```
