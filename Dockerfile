# ──────────────────────────────────────────────────────────────────────────────
# Build context: Flipper/ (parent directory)
# This is required because the `api` crate depends on `lucy` via a local path:
#   lucy = { path = "../Lucy/crates/lucy" }
# Docker builds are hermetic — the path dep must be inside the build context.
# ──────────────────────────────────────────────────────────────────────────────

# ── Stage 0: Build the Lucy React UI ──────────────────────────────────────────
# The lucy-core crate embeds ui/dist/ at compile time via rust-embed.
# We build it in a dedicated Node stage so the Rust stages stay lean.
FROM node:22-slim AS ui-builder
WORKDIR /ui

# Install deps first (cached layer — only re-runs when package-lock.json changes)
COPY Lucy/ui/package.json Lucy/ui/package-lock.json ./
RUN npm ci

# Copy the rest of the UI source and build
COPY Lucy/ui ./
RUN npm run build

# ── Stage 1: dependency planner (cargo-chef) ──────────────────────────────────
FROM lukemathwalker/cargo-chef:0.1.72-rust-1.89.0-slim AS chef
WORKDIR /app

FROM chef AS planner

# Lucy workspace root — needed so crates using `edition.workspace = true` can
# find their inherited fields (Cargo resolves workspace membership by walking up).
COPY Lucy/Cargo.toml  /Lucy/Cargo.toml
COPY Lucy/Cargo.lock  /Lucy/Cargo.lock
COPY Lucy/crates      /Lucy/crates

# Compiled UI — lucy-core embeds it via rust-embed at compile time
COPY --from=ui-builder /ui/dist /Lucy/ui/dist

# Backend workspace manifests and crate sources
COPY Backend/Cargo.toml Backend/Cargo.lock ./
COPY Backend/crates ./crates

RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: dependency builder ───────────────────────────────────────────────
FROM chef AS builder

RUN sed -i 's|http://deb.debian.org|https://cdn-aws.deb.debian.org|g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Lucy deps (same layout as planner)
COPY Lucy/Cargo.toml  /Lucy/Cargo.toml
COPY Lucy/Cargo.lock  /Lucy/Cargo.lock
COPY Lucy/crates      /Lucy/crates
COPY --from=ui-builder /ui/dist /Lucy/ui/dist

COPY --from=planner /app/recipe.json recipe.json

# Compile all *dependencies* (cached layer — only re-runs when deps change)
RUN cargo chef cook --release --recipe-path recipe.json

# Copy the full workspace source and compile the application binary
COPY Backend/Cargo.toml Backend/Cargo.lock ./
COPY Backend/crates ./crates

ARG CRATE=api
RUN cargo build --release --bin ${CRATE}

# ── Stage 3: minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN sed -i 's|http://deb.debian.org|http://cdn-aws.deb.debian.org|g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        wget \
        libssl3 \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system app \
    && useradd --system --gid app --no-create-home app

ARG CRATE=api
COPY --from=builder /app/target/release/${CRATE} /usr/local/bin/app

USER app

EXPOSE 8080

CMD ["app"]
