# headless-rust

Rust + [chromiumoxide](https://github.com/mattsse/chromiumoxide) headless Chromium fetch service.

Same REST contract as `headless-playwright` — the Nuxt blog talks HTTP only.

## Endpoints

| Method | Path | Body | Response |
| --- | --- | --- | --- |
| `GET` | `/health` | — | `{ ok, browser }` |
| `POST` | `/fetch` | `{ url, timeoutMs? }` | `{ ok, title, html, url, latencyMs }` |
| `GET` | `/` | — | service blurb |

## Run

```bash
cp .env.example .env
docker compose up -d --build
curl -s http://127.0.0.1:9381/health
curl -s -X POST http://127.0.0.1:9381/fetch \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com"}'
```

Default host port: **9381** (Playwright 9380, Camoufox 9377, Obscura 9222).

First build compiles chromiumoxide (large) — expect several minutes.

## Blog plugin

`RUST_FETCH_BASE_URL=http://127.0.0.1:9381` — enable the **Rust** fetcher under `/system/browsers`.
