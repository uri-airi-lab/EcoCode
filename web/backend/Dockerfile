# ── Stage 1: Builder ──────────────────────────────────────────────────────────
FROM rust:1.88-bookworm AS builder

WORKDIR /app

# Install system deps needed to compile sqlx (postgres + rustls) and reqwest
RUN apt-get update -o Acquire::Check-Valid-Until=false && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy manifest first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Pre-fetch dependencies with a dummy main so cargo can resolve them
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release 2>/dev/null || true

# Now copy real source and do final build
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update -o Acquire::Check-Valid-Until=false && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/backend ./backend

EXPOSE 3001

CMD ["./backend"]
