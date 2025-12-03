#!/usr/bin/env python3
"""Simple mock update server for local development."""

import argparse
import hashlib
import http.server
import json
import shutil
import socketserver
import sys
from pathlib import Path
from typing import Dict, Optional


class MockUpdateHandler(http.server.BaseHTTPRequestHandler):
    binary_path: Optional[Path] = None
    binary_size: int = 0
    metadata: Dict[str, str] = {}
    download_path: str = "/download"

    def do_GET(self) -> None:  # pragma: no cover - manual test helper
        if self.path.rstrip("/") == "/latest.json":
            payload = json.dumps(self.metadata).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Cache-Control", "no-store, max-age=0")
            self.end_headers()
            self.wfile.write(payload)
            return

        if self.path.rstrip("/") == self.download_path.rstrip("/"):
            if self.binary_path is None:
                self.send_error(500, "Binary path not configured")
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(self.binary_size))
            self.end_headers()
            with self.binary_path.open("rb") as fh:
                shutil.copyfileobj(fh, self.wfile)
            return

        self.send_error(404, "Not Found")

    def log_message(self, format: str, *args) -> None:  # pragma: no cover - no logging needed
        sys.stderr.write("mock-update-server: " + format % args + "\n")


def compute_sha256(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(8192), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def build_metadata(
    version: str, url: str, sha256: str
) -> Dict[str, str]:
    return {"version": version, "download_url": url, "sha256": sha256}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Serve update metadata and the binary for local tests."
    )
    parser.add_argument(
        "--binary",
        "-b",
        required=True,
        help="Path to the Viceroy binary to serve.",
    )
    parser.add_argument(
        "--version",
        "-v",
        default="0.1.0",
        help="Version string to expose in the metadata.",
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Host address to bind.",
    )
    parser.add_argument("--port", "-p", type=int, default=8999, help="Server port.")
    parser.add_argument(
        "--download-path",
        default="/download",
        help="Path used to serve the binary (must match metadata).",
    )
    return parser.parse_args()


def run_server() -> None:
    args = parse_args()
    binary = Path(args.binary).expanduser().resolve()
    if not binary.is_file():
        raise SystemExit(f"{binary} does not exist or is not a file")

    sha = compute_sha256(binary)
    download_url = f"http://{args.host}:{args.port}{args.download_path}"
    metadata = build_metadata(args.version, download_url, sha)

    handler = MockUpdateHandler
    handler.binary_path = binary
    handler.binary_size = binary.stat().st_size
    handler.metadata = metadata
    handler.download_path = args.download_path

    with socketserver.ThreadingTCPServer(
        (args.host, args.port), handler
    ) as httpd:
        httpd.allow_reuse_address = True
        print(f"Serving metadata at http://{args.host}:{args.port}/latest.json")
        print(json.dumps(metadata, indent=2))
        print(f"Binary path: {binary}")
        print()
        print("Run the updater with:")
        print(f"  export VICEROY_UPDATE_METADATA_URL=\"http://{args.host}:{args.port}/latest.json\"")
        print("  cargo run -- --silent-update-check")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nMock update server stopped.")


if __name__ == "__main__":
    run_server()
