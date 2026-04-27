#!/usr/bin/env python3
"""Tiny stdlib-only OpenAI-compat replay/record server for the e2e suite.

Modes:
  replay  (default) - look up fixtures by canonical request hash; 503 if missing
  record            - forward to upstream, persist response, return it

Usage:
  mock-llm-server.py --port 8765 --fixtures e2e/fixtures/llm \
                     [--mode replay|record] [--upstream-url URL] \
                     [--upstream-key TOKEN]
"""
from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import pathlib
import sys
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CONFIG: dict = {}

# Fields that vary harmlessly between calls; drop or normalize before hashing.
_DROP_KEYS = {"stream", "user", "stream_options"}
_ROUND_KEYS = {"temperature", "top_p", "frequency_penalty", "presence_penalty"}


def canonicalize(req: dict) -> dict:
    out = {}
    for k, v in sorted(req.items()):
        if k in _DROP_KEYS:
            continue
        if k in _ROUND_KEYS and isinstance(v, (int, float)):
            out[k] = round(float(v), 2)
        else:
            out[k] = v
    return out


def request_hash(body: bytes) -> str:
    try:
        req = json.loads(body or b"{}")
    except json.JSONDecodeError:
        # Fall back to raw-body hash for non-JSON; still deterministic.
        return hashlib.sha256(body).hexdigest()[:12]
    canon = canonicalize(req)
    canon_bytes = json.dumps(canon, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(canon_bytes).hexdigest()[:12]


def fixture_paths(h: str) -> tuple[pathlib.Path, pathlib.Path]:
    base = pathlib.Path(CONFIG["fixtures"])
    return base / f"{h}.json", base / f"{h}.meta.json"


def forward_upstream(path: str, body: bytes) -> tuple[int, bytes, dict]:
    url = CONFIG["upstream_url"].rstrip("/") + path
    headers = {"Content-Type": "application/json"}
    if CONFIG.get("upstream_key"):
        headers["Authorization"] = f"Bearer {CONFIG['upstream_key']}"
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.status, resp.read(), dict(resp.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read(), dict(e.headers or {})


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):  # noqa: A003 - stdlib API
        sys.stderr.write("[mock-llm] " + (fmt % args) + "\n")

    def _send(self, code: int, body: bytes, ctype: str = "application/json") -> None:
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/healthz":
            self._send(200, b'{"status":"ok"}')
        else:
            self._send(404, b'{"error":"not found"}')

    def do_POST(self) -> None:  # noqa: N802
        if self.path not in ("/v1/chat/completions", "/v1/embeddings"):
            self._send(404, b'{"error":"not found"}')
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if length else b""
        h = request_hash(body)
        body_path, meta_path = fixture_paths(h)

        if CONFIG["mode"] == "replay":
            if body_path.exists():
                self._send(200, body_path.read_bytes())
            else:
                err = {
                    "error": "missing fixture",
                    "hash": h,
                    "path": str(body_path),
                    "hint": "re-record with AETERNA_E2E_LLM_RECORDED_MODE=record",
                }
                self._send(503, json.dumps(err).encode())
            return

        # record mode
        if not CONFIG.get("upstream_url"):
            self._send(500, b'{"error":"record mode requires --upstream-url"}')
            return
        status, resp, _hdrs = forward_upstream(self.path, body)
        if status == 200:
            body_path.write_bytes(resp)
            meta = {
                "hash": h,
                "path": self.path,
                "upstream": CONFIG["upstream_url"],
                "recorded_at": _dt.datetime.utcnow().isoformat() + "Z",
            }
            meta_path.write_text(json.dumps(meta, indent=2) + "\n")
        self._send(status, resp)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--fixtures", required=True)
    ap.add_argument("--mode", choices=("replay", "record"), default="replay")
    ap.add_argument("--upstream-url", default="")
    ap.add_argument("--upstream-key", default="")
    args = ap.parse_args()

    CONFIG.update(
        port=args.port,
        fixtures=args.fixtures,
        mode=args.mode,
        upstream_url=args.upstream_url,
        upstream_key=args.upstream_key or os.environ.get("OPENAI_API_KEY", ""),
    )
    pathlib.Path(args.fixtures).mkdir(parents=True, exist_ok=True)
    httpd = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    sys.stderr.write(
        f"[mock-llm] listening on 127.0.0.1:{args.port} mode={args.mode} fixtures={args.fixtures}\n"
    )
    sys.stderr.flush()
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
