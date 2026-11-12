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

---

## Reference Rust Web Codebases

Idiomatic open-source Rust web applications for study and comparison:

### Small & Focused (Axum)
- **[launchbadge/realworld-axum-sqlx](https://github.com/launchbadge/realworld-axum-sqlx)**: RealWorld (Medium clone) API built by the `sqlx` maintainers. Good reference for central error types (`IntoResponse`), custom auth extractors, and feature-based module layout.
- **[matze/wastebin](https://github.com/matze/wastebin)**: Self-hosted pastebin built with Axum, SQLite, and `askama` HTML templates. Clean example of a small server-rendered web app.
- **[tokio-rs/axum `examples/`](https://github.com/tokio-rs/axum/tree/main/examples)**: Official Axum examples maintained by the Tokio team (`sqlx-postgres`, `error-handling`, `graceful-shutdown`, `customize-extractor-error`).

### Medium Services & Workspace Blueprints (Axum)
- **[atuinsh/atuin (`crates/atuin-server`)](https://github.com/atuinsh/atuin/tree/main/crates/atuin-server)**: Shell history sync server using Axum, SQLx, PostgreSQL, and `tracing`.
- **[maxcountryman/axum-login (`examples/`)](https://github.com/maxcountryman/axum-login/tree/main/examples)**: Session authentication, login/logout flows, and protected route layers by the author of `tower-sessions`.
- **[rust10x/rust-web-app](https://github.com/rust10x/rust-web-app)**: Production Axum blueprint showing how to split a growing app into Cargo workspace crates (`lib-core`, `lib-web`, `lib-auth`, `lib-rpc`).
- **[loco-rs/loco (`examples/`)](https://github.com/loco-rs/loco/tree/master/examples)**: Rails-style framework built on top of Axum and Tower, useful for seeing how background workers, mailers, config, and CLI tasks are structured around Axum.

### Large-Scale Production Systems (Axum)
- **[rust-lang/docs.rs](https://github.com/rust-lang/docs.rs)**: Official Rust documentation host built with Axum, SQLx (with `.sqlx/` offline query cache), server-rendered HTML templates, and background build workers.
- **[windmill-labs/windmill (`backend/`)](https://github.com/windmill-labs/windmill/tree/main/backend)**: Workflow engine and job orchestrator built with Axum, SQLx, and PostgreSQL (`FOR UPDATE SKIP LOCKED` job queues at scale).
- **[svix/svix-webhooks (`server/`)](https://github.com/svix/svix-webhooks/tree/main/server)**: Enterprise webhook delivery service using Axum, PostgreSQL, and Redis (idempotency, outbox/queue workers, and retries).
- **[rust-lang/crates.io](https://github.com/rust-lang/crates.io)**: Official Rust package registry using Axum for HTTP, background job workers, and an integration test harness similar to `TestApp`.
- **[kanidm/kanidm (`server/`)](https://github.com/kanidm/kanidm/tree/master/server)**: Identity and authentication platform built on Axum and Tower with strict type-driven security and structured audit logging.

### Non-Axum Rust Web Codebases (For Comparison)
- **[LukeMathWalker/zero-to-production](https://github.com/LukeMathWalker/zero-to-production)**: Official companion repository for *Zero to Production in Rust* using Actix Web, useful for comparing side-by-side with this Axum port.
- **[LemmyNet/lemmy](https://github.com/LemmyNet/lemmy)**: Federated link aggregator built with Actix Web, Diesel, and PostgreSQL, organized into domain workspace crates.
- **[meilisearch/meilisearch (`crates/meilisearch`)](https://github.com/meilisearch/meilisearch/tree/main/crates/meilisearch)**: Search engine HTTP API built with Actix Web, showcasing clean extractors, pagination, and API error responses.
- **[dani-garcia/vaultwarden](https://github.com/dani-garcia/vaultwarden)**: Bitwarden-compatible password manager server built with Rocket, known for staying compact and readable.

