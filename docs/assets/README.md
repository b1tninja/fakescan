# Brand assets

The mark is a flatbed with a page on the glass, a scan bar through the page, and **Fs** on the chin. The wordmark is `fake` in cyan-to-blue and `scan` in the page color.

| File | Use |
|------|-----|
| [`fakescan-logo.svg`](fakescan-logo.svg) | Banner in the root README |
| [`fakescan-mark.svg`](fakescan-mark.svg) | App icon tile |
| [`fakescan.ico`](fakescan.ico) | Windows `fakescan.exe` (multi-size) |
| [`fakescan.png`](fakescan.png) | 256×256 raster of the tile |
| [`render_icons.py`](render_icons.py) | Rasterize the mark SVG into `.ico` / `.png` |

```bash
python docs/assets/render_icons.py
```
