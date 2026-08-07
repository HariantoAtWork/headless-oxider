# headless-rust (stealth)

Rust + [chaser-oxide](https://github.com/ccheshirecat/chaser-oxide) Chromium fetch service with **stealth on by default**.

Same REST contract as `headless-playwright` — the Nuxt blog talks HTTP only. Page loads stay **inside Chromium** (honest Chrome TLS / JA3); we do not MITM HTML through a separate HTTP client.

## Endpoints

| Method | Path | Body | Response |
| --- | --- | --- | --- |
| `GET` | `/health` | — | `{ ok, browser, stealth, profile, blocklist }` |
| `POST` | `/fetch` | `{ url, timeoutMs?, sessionKey?, proxy?, stealth? }` | `{ ok, title, html, url, latencyMs, stealth, profile }` |
| `GET` | `/` | — | service blurb |

## Stealth stack

| Layer | What |
| --- | --- |
| Protocol | chaser-oxide CDP transport patches / isolated-world eval |
| Fingerprint | `FINGERPRINT_PROFILE=native\|linux\|windows\|macos\|rotate` (consistent presets, not random noise) |
| Automation signals | webdriver / HeadlessChrome / cdc_ scrubbing via chaser profiles |
| Trackers | `Network.setBlockedURLs` + [`data/blocklist.txt`](data/blocklist.txt) |
| Behaviour | short Bezier mouse + scroll settle before HTML extract |
| Proxy | `PROXY_URL` or per-request `proxy` |
| Sticky | `sessionKey` → persistent user-data-dir under `SESSIONS_DIR` |
| Headful | compose profile `headful` runs under Xvfb |

`STEALTH=0` disables profile/blocklist/humanise (debug only).

## Run

```bash
cp .env.example .env
docker compose up -d --build
curl -s http://127.0.0.1:9381/health | jq .
curl -s -X POST http://127.0.0.1:9381/fetch \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com"}' | jq '{title, stealth, profile, latencyMs}'
```

Headful / Xvfb:

```bash
docker compose --profile headful up -d --build
```

Do not export `HEADFUL=1` for the default service — that container has no display. The headful service already sets `HEADFUL=1` itself.

Default host ports: **9381** (headless), **9382** (headful / Xvfb). Related stack: Playwright 9380, Camoufox 9377, Obscura 9222.

First build compiles chaser-oxide / CDP bindings — expect several minutes.

## Stealth smoke matrix (manual)

Run the same URLs through Obscura, Camoufox, and this service; record pass/fail:

| Probe | Expect (stealth=1) | vs Obscura | vs Camoufox |
| --- | --- | --- | --- |
| `https://example.com` | HTML + title | parity | parity |
| bot.sannysoft.com / areyouheadless-style | fewer failed automation rows than stock Chromium | should **beat** Obscura on CDP/protocol leaks | Camoufox may still win on Firefox-native FP |
| Cloudflare passive / WAF soft checks | often OK with residential `PROXY_URL` | competitive | competitive |
| Cloudflare interactive / DataDome / Akamai | **not guaranteed** | same class | Camoufox also not magic alone |

### Honest limits

- This is **best-effort Chromium protocol stealth**, not a patched Firefox engine.
- Camoufox still leads on **C++ Firefox fingerprinting** + Juggler isolation.
- IP reputation needs **your** proxy; the browser cannot invent a clean IP.
- No claim of “undetectable everywhere”.

## Env

| Var | Default | Meaning |
| --- | --- | --- |
| `STEALTH` | `1` | Master stealth switch |
| `FINGERPRINT_PROFILE` | `native` | Profile preset / `rotate` |
| `HEADFUL` | `0` | Headed Chrome (use Xvfb profile) |
| `PROXY_URL` | — | Launch-time proxy |
| `CHROME_PATH` | `/usr/bin/chromium` | Browser binary |
| `BLOCKLIST_PATH` | `/etc/headless-rust/blocklist.txt` | Extra URL patterns |
| `SESSIONS_DIR` | `/var/lib/headless-rust/sessions` | Sticky profiles |

## Blog plugin

`RUST_FETCH_BASE_URL=http://127.0.0.1:9381` — enable **Rust (Chromium)** under `/system/browsers`.
