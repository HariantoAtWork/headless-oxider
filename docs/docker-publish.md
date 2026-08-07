# Docker publish

How images get from this repo to [Docker Hub](https://hub.docker.com/r/hariantoatwork/headless-oxider).

## Short version

1. You push a **git tag** like `v1.2.0`.
2. GitHub Actions builds the image for **`linux/amd64`** and **`linux/arm64`**.
3. Docker Hub gets tags such as `1.2.0`, `1.2`, and `latest`.

GitHub does **not** invent version numbers. No `v*` tag → no new published version.

## What triggers a publish

Workflow: [`.github/workflows/docker-publish.yml`](../.github/workflows/docker-publish.yml)

| Trigger | What happens |
| --- | --- |
| Push tag matching `v*` | Full publish; Docker tags include `latest` |
| Manual **workflow_dispatch** | Build/push runs; `latest` is **not** set (only applies on `v*` tags) |
| Push to `main` only | Nothing published |

### Release a new image

```bash
git tag v1.2.0
git push origin v1.2.0
```

SemVer with a leading `v` is expected (`v1.2.0`, not `1.2.0`).

### Required secrets

Repository secrets (Settings → Secrets and variables → Actions):

| Secret | Purpose |
| --- | --- |
| `DOCKERHUB_USERNAME` | Docker Hub namespace (also used as the image path) |
| `DOCKERHUB_TOKEN` | Docker Hub access token (not your password) |

Image name: `${DOCKERHUB_USERNAME}/headless-oxider`.

## Git tag → Docker tags

[`docker/metadata-action`](https://github.com/docker/metadata-action) derives Hub tags from the git tag:

| Git ref | Docker Hub tags |
| --- | --- |
| `v1.2.0` | `1.2.0`, `1.2`, `latest` |
| `v1.1.1` | `1.1.1`, `1.1`, `latest` (when that tag is the one being published) |

Pull examples:

```bash
docker pull hariantoatwork/headless-oxider:latest
docker pull hariantoatwork/headless-oxider:1.2.0
docker pull hariantoatwork/headless-oxider:1.2
```

One Hub tag is a **multi-arch manifest**. Clients pull `amd64` or `arm64` automatically.

## Multi-platform builds

The workflow uses:

- `docker/setup-qemu-action` — emulate non-native arches on the runner
- `docker/setup-buildx-action` — Buildx builder
- `platforms: linux/amd64,linux/arm64` on `docker/build-push-action`

GitHub-hosted `ubuntu-latest` is **amd64**. That arch builds natively; **arm64** is built under QEMU (slower, especially for Rust).

Your Mac (Apple Silicon) is arm64. That does **not** publish images by itself — only this workflow (or a local `docker buildx build --push`) does.

## When to cut a new tag

| Change | New `v*` tag? |
| --- | --- |
| `Dockerfile`, Rust source, `Cargo.toml` / lockfile, `data/blocklist.txt`, runtime deps | **Yes** — users need a new image |
| Compose / `.env.example` that affect how people run the image | Usually **yes** if behaviour changes |
| README, docs, changelog, workflow tweaks that do not change the image layers | **No** |
| Multi-arch / CI-only workflow change | Optional republish if you want Hub images rebuilt with the new pipeline |

Rule of thumb: if the bits inside the container change, tag; if only documentation or CI glue changes, skip.

## Local multi-arch (optional)

Same idea as CI, from a machine with Docker Buildx:

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t "$DOCKERHUB_USERNAME/headless-oxider:dev" \
  --push .
```

Prefer CI tags for anything users should pull.

## Checklist before tagging

1. Changes that belong in the image are committed and pushed to `main`.
2. Secrets `DOCKERHUB_USERNAME` / `DOCKERHUB_TOKEN` are set.
3. Choose the next SemVer (`vMAJOR.MINOR.PATCH`).
4. `git tag v…` then `git push origin v…`.
5. Confirm the Actions run is green and Hub shows the new tags (including amd64 + arm64).
