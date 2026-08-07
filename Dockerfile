# Multi-stage: compile Rust service, run with system Chromium.
FROM rust:1.85-bookworm AS builder
WORKDIR /app

# Cache dependencies
COPY Cargo.toml ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    ca-certificates \
    fonts-liberation \
    fonts-noto-color-emoji \
    libnss3 \
    libatk-bridge2.0-0 \
    libgtk-3-0 \
    libx11-xcb1 \
    libxcomposite1 \
    libxdamage1 \
    libxrandr2 \
    libgbm1 \
    libasound2 \
    libpangocairo-1.0-0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/headless-rust /usr/local/bin/headless-rust

ENV RUST_FETCH_PORT=9381
ENV RUST_FETCH_HOST=0.0.0.0
ENV CHROME_PATH=/usr/bin/chromium

EXPOSE 9381

CMD ["headless-rust"]
