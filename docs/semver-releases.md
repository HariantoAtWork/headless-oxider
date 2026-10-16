# SemVer releases (learn by shipping)

This is how **this** repo publishes to Docker Hub. The same idea applies to the other `headless-*` images. Read this once slowly; then use the checklist every time you cut a release.

## Why SemVer exists

Users and compose files pin versions. A version string is a **contract**:

```text
MAJOR.MINOR.PATCH   →   e.g. 1.2.0
```

| Part | Bump when | Example |
| --- | --- | --- |
| **MAJOR** | Breaking change for anyone already running the image | Remove an HTTP endpoint, change default port, rename required env vars |
| **MINOR** | New capability, old usage still works | Add `/pdf` while `/fetch` stays the same |
| **PATCH** | Fix or small internal change, same behaviour for clients | Bugfix, dependency bump, Dockerfile tweak that does not change the API |

Rule of thumb: **think from the puller’s point of view** (`docker pull …:1.2.0` and their compose). If their setup can break without them changing config, that is MAJOR. If they gain something optional, MINOR. If you only repaired something, PATCH.

Official spec (short): [semver.org](https://semver.org/).

## How *our* pipeline uses SemVer

You choose the version. GitHub Actions does **not**.

```text
git tag v1.2.0  ──push──►  Actions build  ──►  Docker Hub
                                              1.2.0
                                              1.2
                                              latest
```

| You create (git) | Hub gets |
| --- | --- |
| `v1.2.0` | `harianto/headless-oxider:1.2.0`, `:1.2`, `:latest` |
| `v2.0.0` | `:2.0.0`, `:2.0`, `:latest` |

Notes that trip people up:

1. The git tag has a leading **`v`**. Hub tags do **not**.
2. Pushing `main` alone publishes **nothing**.
3. `latest` moves to whatever `v*` tag you just pushed. Pin compose to `1.2.0` (or `1.2`) in production if you care about stability.
4. Moving / force-pushing a tag that already existed on Hub is a bad habit — cut `v1.2.1` instead.

Workflow file: [`.github/workflows/docker-publish.yml`](../.github/workflows/docker-publish.yml).  
Mechanics (secrets, multi-arch, when *not* to tag): [docker-publish.md](./docker-publish.md).

## Decide the next version

```bash
git fetch --tags
git tag -l 'v*' | sort -V | tail -5
```

Look at **what changed since the last tag** (not since yesterday):

```bash
git log v1.2.0..HEAD --oneline   # replace with your latest tag
```

Then pick **one** bump:

| Since last tag you… | Next tag |
| --- | --- |
| Only fixed bugs / deps / image internals | `v1.2.1` (patch) |
| Added features, still compatible | `v1.3.0` (minor) |
| Broke or renamed the contract | `v2.0.0` (major) |

Do **not** skip the decision. “Just put a higher number” without knowing *why* teaches bad habits.

If **nothing** in the image changed (docs-only, README, changelog prose), **do not tag**. Docs can live on `main` without a Hub republish.

## Release checklist (copy each time)

On a clean `main`, after the code you want published is pushed:

```bash
# 1. Confirm you are releasing what you think
git status
git log -1 --oneline
git fetch origin && git pull --ff-only origin main

# 2. Choose the version (example: patch)
NEXT=v1.2.1

# 3. Tag the current commit (annotated tag — preferred)
git tag -a "$NEXT" -m "$NEXT"

# 4. Push the tag (this triggers CI → Docker Hub)
git push origin "$NEXT"

# 5. Confirm
#    GitHub → Actions → "Publish Docker image" green
#    Docker Hub → harianto/headless-oxider shows 1.2.1 / 1.2 / latest
```

Pull to verify:

```bash
docker pull harianto/headless-oxider:1.2.1
```

Keep compose examples in the repo in sync when you intentionally move the “current” documented version (e.g. README `1.2.0` → `1.2.1`).

## Habits of a careful releaser

- **One meaning per release.** Mix a breaking API change with five unrelated fixes → still MAJOR; say so in the tag message / changelog.
- **Tag the commit that contains the change**, after it is on `main`. Do not tag a dirty worktree “to test”.
- **Prefer annotated tags** (`git tag -a`) — they store message + author; CI only needs the name `v*`.
- **Never rewrite published history for versions people already pulled.** New patch tag instead.
- **Pin in production** (`image: …:1.2.0`). Use `latest` only for throwaway labs.
- **Secrets stay in GitHub Actions secrets**, not in git. Hub username + access token: `DOCKERHUB_USERNAME`, `DOCKERHUB_TOKEN`.

## Sibling images (same ritual)

| Repo | Image | Example tag |
| --- | --- | --- |
| [headless-oxider](https://github.com/HariantoAtWork/headless-oxider) | `harianto/headless-oxider` | `v1.2.0` → `1.2.0` |
| [headless-camoufox](https://github.com/HariantoAtWork/headless-camoufox) | `harianto/headless-camoufox` | `v1.0.0` |
| [headless-obscura](https://github.com/HariantoAtWork/headless-obscura) | `harianto/headless-obscura` | `v1.0.0` (amd64 only) |
| [headless-playwright](https://github.com/HariantoAtWork/headless-playwright) | `harianto/headless-playwright` | `v1.0.0` |
| [headless-tunnel](https://github.com/HariantoAtWork/headless-tunnel) | `harianto/headless-tunnel` | `v1.0.0` |

Same SemVer rules; only image name and platforms differ.

## Quick self-test

1. You fixed a crash in `/fetch`. Last tag was `v1.2.0`. What do you push? → `v1.2.1`
2. You add `/screenshot` without changing `/fetch`. → `v1.3.0`
3. You rename `POST /fetch` to `POST /html` and remove `/fetch`. → `v2.0.0`
4. You only edit this markdown file. → no tag

If those four are obvious, you are doing SemVer properly.
