#!/usr/bin/env bash
# Quick stealth smoke against a running headless-oxider (default :9381).
set -euo pipefail
BASE="${1:-http://127.0.0.1:9381}"

echo "== GET /health =="
curl -fsS "$BASE/health" | tee /tmp/hr-health.json
echo

echo "== POST /fetch example.com =="
curl -fsS -X POST "$BASE/fetch" \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com","timeoutMs":30000}' \
  | tee /tmp/hr-fetch.json \
  | head -c 400
echo
echo "…"

python3 - <<'PY'
import json
h=json.load(open("/tmp/hr-health.json"))
assert h.get("ok") is True, h
assert "stealth" in h and "profile" in h, h
f=json.load(open("/tmp/hr-fetch.json"))
assert f.get("ok") is True, f
assert "example" in (f.get("title") or "").lower() or "example" in (f.get("html") or "").lower()
print("smoke OK — stealth=%s profile=%s latencyMs=%s" % (f.get("stealth"), f.get("profile"), f.get("latencyMs")))
PY
