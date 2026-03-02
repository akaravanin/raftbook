# syntax=docker/dockerfile:1

# ── Stage 1: install cargo-chef ───────────────────────────────────────────────
FROM rust:latest AS chef
# protobuf-compiler (protoc) is required by tonic-build to compile .proto files.
RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
WORKDIR /app

# ── Stage 2: compute dependency recipe ────────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 3: cache deps, then build binary ────────────────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# This layer is invalidated only when dependencies change, not source files.
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release -p engined

# ── Stage 4: minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
# ca-certificates is required by rustls for TLS peer verification.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 10001 raftbook
WORKDIR /app
COPY --from=builder /app/target/release/engined /usr/local/bin/engined
USER raftbook
ENTRYPOINT ["/usr/local/bin/engined"]
