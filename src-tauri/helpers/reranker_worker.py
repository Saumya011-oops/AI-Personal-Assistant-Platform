#!/usr/bin/env python3

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from sentence_transformers import CrossEncoder


parser = argparse.ArgumentParser()
parser.add_argument("--port", type=int, default=8742)
parser.add_argument("--model", default="cross-encoder/ms-marco-MiniLM-L-6-v2")
args = parser.parse_args()

MODEL = CrossEncoder(args.model)


class Handler(BaseHTTPRequestHandler):
    def _write_json(self, status, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            self._write_json(200, {"ready": True, "model": args.model})
            return
        self._write_json(404, {"error": "Not found"})

    def do_POST(self):
        if self.path != "/rerank":
            self._write_json(404, {"error": "Not found"})
            return

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        payload = json.loads(raw.decode("utf-8"))

        query = str(payload.get("query", "")).strip()
        chunks = payload.get("chunks", [])
        limit = int(payload.get("limit", 10))

        if not query or not chunks:
            self._write_json(200, {"results": []})
            return

        pairs = [(query, chunk.get("content", "")) for chunk in chunks]
        scores = MODEL.predict(pairs)
        ranked = sorted(
            [
                {
                    "chunkId": chunk.get("chunkId"),
                    "score": float(score),
                }
                for chunk, score in zip(chunks, scores)
            ],
            key=lambda item: item["score"],
            reverse=True,
        )[:limit]

        self._write_json(200, {"results": ranked})


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", args.port), Handler).serve_forever()
