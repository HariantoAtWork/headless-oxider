# Multi-stage: compile Rust stealth service, run with system Chromium (+ optional Xvfb).
FROM rust:1.88-bookworm AS builder
WORKDIR /app

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
    xvfb \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/headless-rust /usr/local/bin/headless-rust
COPY data/blocklist.txt /etc/headless-rust/blocklist.txt
RUN mkdir -p /var/lib/headless-rust/sessions

ENV RUST_FETCH_PORT=9381
ENV RUST_FETCH_HOST=0.0.0.0
ENV CHROME_PATH=/usr/bin/chromium
ENV STEALTH=1
ENV HEADFUL=0
ENV FINGERPRINT_PROFILE=native
ENV BLOCKLIST_PATH=/etc/headless-rust/blocklist.txt
ENV SESSIONS_DIR=/var/lib/headless-rust/sessions

EXPOSE 9381

CMD ["headless-rust"]
