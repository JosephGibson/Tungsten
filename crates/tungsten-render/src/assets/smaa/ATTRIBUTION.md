# SMAA Lookup Textures — Attribution

The binary lookup textures `area.bin` and `search.bin` are derived from the
upstream SMAA reference implementation by Jorge Jimenez et al.

## Source

- Project: [iryoku/smaa](https://github.com/iryoku/smaa)
- Paper: [SMAA: Enhanced Subpixel Morphological Antialiasing](https://www.iryoku.com/smaa/downloads/SMAA-Enhanced-Subpixel-Morphological-Antialiasing.pdf)
- Site: <https://www.iryoku.com/smaa/>

## Generation

The two `.bin` files are the raw byte-arrays embedded in the upstream C
headers, written out without re-encoding:

- `area.bin`: bytes from `Textures/AreaTex.h`'s `areaTexBytes`. Format
  `Rg8Unorm`, 160 × 560, 179 200 bytes.
- `search.bin`: bytes from `Textures/SearchTex.h`'s `searchTexBytes`. Format
  `R8Unorm`, 64 × 16, 1 024 bytes.

These files are loaded at runtime via `include_bytes!` and uploaded to GPU
textures by `crates/tungsten-render/src/post/smaa_luts.rs`. They are
intentionally kept out of `assets/manifest.json`: they are engine-internal
content, not user-tweakable assets.

## License — MIT

Copyright (C) 2013 Jorge Jimenez (jorge@iryoku.com)
Copyright (C) 2013 Jose I. Echevarria (joseignacioechevarria@gmail.com)
Copyright (C) 2013 Belen Masia (bmasia@unizar.es)
Copyright (C) 2013 Fernando Navarro (fernandn@microsoft.com)
Copyright (C) 2013 Diego Gutierrez (diegog@unizar.es)

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software. As clarification, there is no
requirement that the copyright notice and permission be included in binary
distributions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
