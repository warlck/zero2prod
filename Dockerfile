# ---------------------------------------------------
# Stage 1: Base Chef stage (pre-installed cargo-chef on Alpine)
# ---------------------------------------------------
FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
WORKDIR /app
RUN apk add --no-cache musl-dev

# ---------------------------------------------------
# Stage 2: Planner stage (computes recipe.json)
# ---------------------------------------------------
FROM chef AS planner
COPY . .
# Compute lockfile and dependencies recipe skeleton
RUN cargo chef prepare --recipe-path recipe.json

# ---------------------------------------------------
# Stage 3: Builder stage (cooks dependencies & builds binary)
# ---------------------------------------------------
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build and cache dependencies - this layer stays cached until Cargo.toml/Cargo.lock changes!
RUN cargo chef cook --release --recipe-path recipe.json

# Now copy your actual source code
COPY . .
ENV SQLX_OFFLINE=true
# Compile only our application binary
RUN cargo build --release --bin zero2prod

# ---------------------------------------------------
# Stage 4: Runtime stage (pure minimal Alpine)
# ---------------------------------------------------
FROM alpine:latest AS runtime
WORKDIR /app

# Install ca-certificates for TLS support
RUN apk add --no-cache ca-certificates

# Copy compiled binary from builder
COPY --from=builder /app/target/release/zero2prod zero2prod

# Copy runtime configuration directory
COPY configuration configuration

# Set environment
ENV APP_ENVIRONMENT=production

EXPOSE 8000
ENTRYPOINT ["./zero2prod"]
