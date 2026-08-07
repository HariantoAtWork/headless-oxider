# Minimal run (published image)

Pull and run with almost no config. The image already sets sensible defaults (`STEALTH=1`, `HEADFUL=0`, listen on `0.0.0.0:9381`, native fingerprint, built-in blocklist).

## Compose file

Ready-to-copy: [`docker-compose.image.yml`](docker-compose.image.yml)

```yaml
services:
  headless-oxider:
    image: harianto/headless-oxider:1.1.1
    ports:
      - "9381:9381"
    shm_size: "1gb"
    restart: unless-stopped

  headless-oxider-headful:
    profiles: ["headful"]
    image: harianto/headless-oxider:1.1.1
    ports:
      - "9382:9381"
    environment:
      HEADFUL: "1"
    shm_size: "1gb"
    entrypoint:
      - sh
      - -c
      - |
        Xvfb :99 -screen 0 1920x1080x24 -nolisten tcp &
        export DISPLAY=:99
        i=0
        while [ $$i -lt 50 ]; do
          [ -S /tmp/.X11-unix/X99 ] && break
          i=$$((i + 1))
          sleep 0.1
        done
        exec headless-oxider
    restart: unless-stopped
```

For the default (headless) service, `shm_size` is the only non-obvious bit — Chromium needs shared memory or tabs often crash. No other `environment:` is required unless you override defaults.

The headful service only sets `HEADFUL=1` and a small Xvfb entrypoint (same pattern as the root compose). It stays behind the `headful` profile so a normal `up` does not start it.

## Run

Headless only (port **9381**):

```bash
docker compose -f docs/docker-compose.image.yml up -d
curl -s http://127.0.0.1:9381/health | jq .
```

Headless + headful/Xvfb (ports **9381** and **9382**):

```bash
docker compose -f docs/docker-compose.image.yml --profile headful up -d
curl -s http://127.0.0.1:9382/health | jq .
```

Or copy the file next to your other stacks and drop the `docs/` path.

## Image defaults (no compose env needed)

| Variable | Default in image |
| --- | --- |
| `RUST_FETCH_PORT` | `9381` |
| `RUST_FETCH_HOST` | `0.0.0.0` |
| `CHROME_PATH` | `/usr/bin/chromium` |
| `STEALTH` | `1` |
| `HEADFUL` | `0` |
| `FINGERPRINT_PROFILE` | `native` |
| `BLOCKLIST_PATH` | `/etc/headless-oxider/blocklist.txt` |
| `SESSIONS_DIR` | `/var/lib/headless-oxider/sessions` |

Optional overrides only when you need them:

```yaml
services:
  headless-oxider:
    image: harianto/headless-oxider:1.1.1
    ports:
      - "9381:9381"
    shm_size: "1gb"
    environment:
      PROXY_URL: "http://user:pass@proxy.example:8080"
      FINGERPRINT_PROFILE: windows
    volumes:
      - oxider-sessions:/var/lib/headless-oxider/sessions
    restart: unless-stopped

volumes:
  oxider-sessions:
```

## vs repo compose

| | Minimal image compose | Root `docker-compose.yml` |
| --- | --- | --- |
| Source | Docker Hub `image:` | `build:` + `image:` |
| Env | Image defaults (+ `HEADFUL=1` on profile) | Explicit env mapping from `.env` |
| Sessions volume | Optional | Included |
| Headful profile | Yes (Xvfb entrypoint) | Yes |
