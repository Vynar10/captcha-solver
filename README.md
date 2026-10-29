# captcha-solver

A Rust project for learning reqwest, serde, tokio and some basic image processing.

`grid-challenge/` is a small local server that serves image-grid challenges: 9 images and a prompt like "select every circle". It was written by AI as a practice target. The solver in `src/` is mine. It fetches a challenge, works out which images match, and submits the answer.

## How the solver works

1. Decode the base64 PNGs.
2. Threshold each image into a black/white mask (the shapes are dark, the background is light).
3. Apply a morphological opening to remove the thin noise lines.
4. Flood-fill to keep only the largest blob.
5. Measure two ratios and classify the shape:
   - **fill ratio**: shape area ÷ area of the circle around it. Separates circles (~0.92+), squares/crosses (~0.64) and triangles (~0.42).
   - **edge ratio**: nearest background pixel ÷ farthest shape pixel, measured from the centre. Separates squares (~0.71) from crosses (~0.45).

## Running

Start the challenge server (needs Python 3 and Pillow):

```sh
cd grid-challenge
python3 server.py
```

Then, in another terminal, run the solver:

```sh
cargo run
```

It solves `ROUNDS` challenges (set in `src/main.rs`) and prints the score.
