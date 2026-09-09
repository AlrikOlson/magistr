# Image fixtures

Real encoder output for the two formats `StreamImage::decode` reads that
this build cannot also *write* — a test that round-trips a decoder
through its own crate's encoder proves less than one that reads what a
real encoder emitted.

| File | What it is | What it proves |
|------|------------|----------------|
| `half.jpg` | 40x60, left half red, right half blue, baseline JPEG at quality 80 | the zune-jpeg path, and that 40x60 lands on the same `(4, 3)` footprint the PNG test asserts |
| `lossy.webp` | the same image, lossy VP8 at quality 85, no alpha | the VP8 path and the RGB8 -> RGBA8 expansion |
| `alpha.webp` | 2x1, half-alpha red beside opaque blue, lossless VP8L | the VP8L path and that straight alpha survives into `rgba()` |
| `wide.webp` | 16383x1 solid colour, lossless VP8L, 44 bytes | the `MAX_IMAGE_PIXELS` ceiling refusing from the header, before any allocation |

Regenerated with (`cwebp` from libwebp, `sips` from macOS):

```
sips -s format jpeg -s formatOptions 80 src.png --out half.jpg
cwebp -q 85 src.png -o lossy.webp
cwebp -lossless -exact alpha.png -o alpha.webp
cwebp -lossless wide.png -o wide.webp
```
