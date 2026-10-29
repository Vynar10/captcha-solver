"""Local image-grid challenge server for practicing automation.

Each challenge is a 3x3 grid of generated PNGs and a prompt like "circle".
Select every image containing that shape. Run: python3 server.py [port]

API:
  GET  /api/challenge  -> {"id", "prompt", "images": [9 base64 PNGs]}
  POST /api/answer     <- {"id", "selected": [indices]}
                       -> {"correct", "expected", "selected"}
"""

import base64
import io
import json
import math
import random
import sys
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SHAPES = ["circle", "square", "triangle", "cross"]
SIZE = 128
CHALLENGE_TTL = 120  # seconds
PAGE = Path(__file__).with_name("index.html")

# id -> (expected indices, created timestamp); each challenge is single-use
challenges: dict[str, tuple[list[int], float]] = {}


def random_color(dark: bool) -> tuple[int, int, int]:
    lo, hi = (0, 110) if dark else (150, 255)
    return tuple(random.randint(lo, hi) for _ in range(3))


def rotated(points, cx, cy, angle):
    c, s = math.cos(angle), math.sin(angle)
    return [(cx + (x - cx) * c - (y - cy) * s, cy + (x - cx) * s + (y - cy) * c) for x, y in points]


def draw_shape(draw: ImageDraw.ImageDraw, shape: str) -> None:
    r = random.randint(22, 40)
    cx = random.randint(r + 8, SIZE - r - 8)
    cy = random.randint(r + 8, SIZE - r - 8)
    angle = random.uniform(0, math.tau)
    fill = random_color(dark=True)

    if shape == "circle":
        draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=fill)
    elif shape == "square":
        pts = [(cx - r, cy - r), (cx + r, cy - r), (cx + r, cy + r), (cx - r, cy + r)]
        draw.polygon(rotated(pts, cx, cy, angle), fill=fill)
    elif shape == "triangle":
        pts = [(cx + r * math.cos(angle + i * math.tau / 3), cy + r * math.sin(angle + i * math.tau / 3)) for i in range(3)]
        draw.polygon(pts, fill=fill)
    elif shape == "cross":
        w = r // 3
        for pts in (
            [(cx - r, cy - w), (cx + r, cy - w), (cx + r, cy + w), (cx - r, cy + w)],
            [(cx - w, cy - r), (cx + w, cy - r), (cx + w, cy + r), (cx - w, cy + r)],
        ):
            draw.polygon(rotated(pts, cx, cy, angle), fill=fill)


def make_image(shape: str) -> str:
    img = Image.new("RGB", (SIZE, SIZE), random_color(dark=False))
    draw = ImageDraw.Draw(img)
    # background noise so simple pixel counting isn't enough
    for _ in range(random.randint(20, 60)):
        x, y = random.randint(0, SIZE), random.randint(0, SIZE)
        draw.line((x, y, x + random.randint(-15, 15), y + random.randint(-15, 15)),
                  fill=random_color(dark=random.random() < 0.3), width=1)
    draw_shape(draw, shape)
    img = img.filter(ImageFilter.GaussianBlur(random.uniform(0, 0.8)))

    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return base64.b64encode(buf.getvalue()).decode()


def new_challenge() -> dict:
    now = time.time()
    for cid in [c for c, (_, t) in challenges.items() if now - t > CHALLENGE_TTL]:
        del challenges[cid]

    target = random.choice(SHAPES)
    others = [s for s in SHAPES if s != target]
    n_target = random.randint(2, 5)
    shapes = [target] * n_target + [random.choice(others) for _ in range(9 - n_target)]
    random.shuffle(shapes)

    cid = uuid.uuid4().hex
    challenges[cid] = ([i for i, s in enumerate(shapes) if s == target], now)
    return {"id": cid, "prompt": target, "images": [make_image(s) for s in shapes]}


class Handler(BaseHTTPRequestHandler):
    def send_json(self, status: int, body: dict) -> None:
        payload = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self) -> None:
        if self.path in ("/", "/index.html"):
            page = PAGE.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(page)))
            self.end_headers()
            self.wfile.write(page)
        elif self.path == "/api/challenge":
            self.send_json(200, new_challenge())
        else:
            self.send_error(404)

    def do_POST(self) -> None:
        if self.path != "/api/answer":
            self.send_error(404)
            return
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
            cid, selected = body["id"], sorted(set(int(i) for i in body["selected"]))
        except (ValueError, KeyError, TypeError) as e:
            self.send_json(400, {"error": f"bad request body: {e}"})
            return

        entry = challenges.pop(cid, None)
        if entry is None:
            self.send_json(404, {"error": "unknown or expired challenge id"})
            return
        expected, created = entry
        if time.time() - created > CHALLENGE_TTL:
            self.send_json(410, {"error": "challenge expired"})
            return

        self.send_json(200, {"correct": selected == expected, "expected": expected, "selected": selected})


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8081
    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"Serving on http://localhost:{port}")
    server.serve_forever()
