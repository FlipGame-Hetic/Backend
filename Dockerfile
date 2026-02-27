FROM lukemathwalker/cargo-chef:latest-rust-1.83-slim AS chef
WORKDIR /app

FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

RUN sed -i 's|http://deb.debian.org|https://cdn-aws.deb.debian.org|g' /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

ARG CRATE=api
RUN cargo build --release --bin ${CRATE}


# ---------- Runtime minimal ----------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        wget \
        libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system app \
    && useradd --system --gid app --no-create-home app

ARG CRATE=api
COPY --from=builder /app/target/release/${CRATE} /usr/local/bin/app

USER app

EXPOSE 8080

CMD ["app"]