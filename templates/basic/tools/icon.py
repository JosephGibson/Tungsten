"""The game's Windows icon: the gem's diamond on the theme's background,
drawn with `sprites.py`'s shape sampler at 16, 32, 48 and 256 px, each
PNG-encoded, into `windows/icon.ico` and `windows/icon.res`.

The `.res` holds one `RT_GROUP_ICON` named 1 over four `RT_ICON` entries
named 1 to 4, language neutral, no version strings; `build.rs` hands it to
`link.exe` for a Windows MSVC build. `--check` compares decoded pixels with
a fresh render, as `sprites.py`'s does, since deflate's bytes differ between
zlib builds: every compressed size, offset and byte count is held to the
payload it describes, and the fixed fields (types, IDs, counts, dimensions,
language) to a fresh render's.

    python3 -B tools/icon.py            # write windows/
    python3 -B tools/icon.py --out DIR  # write DIR instead
    python3 -B tools/icon.py --check    # compare windows/ with a fresh render
"""
import argparse
import pathlib
import struct
import sys
import zlib

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import sprites  # noqa: E402

WINDOWS = pathlib.Path(__file__).resolve().parent.parent / 'windows'
SIZES = (16, 32, 48, 256)
# The theme's background token, `#0b0c10`.
BACKGROUND = (0x0b, 0x0c, 0x10, 255)
# The gem's share of the icon's side.
SHARE = 0.84
RT_ICON, RT_GROUP_ICON = 3, 14
# Moveable and discardable; the group pure as well, as resource compilers
# write them.
ICON_FLAGS, GROUP_FLAGS = 0x1010, 0x1030


def draw(size):
    """The icon at `size` px square: RGBA rows."""
    _, _, gem = sprites.gem()
    scale = size * SHARE / 20
    offset = (size - 20 * scale) / 2
    layers = [(sprites.box(0, 0, size, size), BACKGROUND)]
    for shape, color in gem:
        moved = lambda x, y, shape=shape: shape((x - offset) / scale, (y - offset) / scale)
        layers.append((moved, color))
    return sprites.render(size, size, layers)


def ico(pngs):
    """An ICO file of `pngs`, one per size of [`SIZES`]."""
    head = struct.pack('<HHH', 0, 1, len(pngs))
    entries, at = b'', 6 + 16 * len(pngs)
    for size, png in zip(SIZES, pngs):
        entries += struct.pack('<BBBBHHII', size % 256, size % 256, 0, 0, 1, 32, len(png), at)
        at += len(png)
    return head + entries + b''.join(pngs)


def resource(kind, name, flags, data):
    """One `.res` entry: a 32-byte header of ordinals, then `data` padded to
    four bytes."""
    header = struct.pack('<IIHHHHIHHII', len(data), 32, 0xFFFF, kind, 0xFFFF, name, 0, flags,
                         0, 0, 0)
    return header + data + bytes(-len(data) % 4)


def group(pngs):
    """The `RT_GROUP_ICON` data over the `RT_ICON` entries 1 to 4."""
    entries = b''.join(
        struct.pack('<BBBBHHIH', size % 256, size % 256, 0, 0, 1, 32, len(png), at + 1)
        for at, (size, png) in enumerate(zip(SIZES, pngs)))
    return struct.pack('<HHH', 0, 1, len(pngs)) + entries


def res(pngs):
    """A `.res` file: the empty entry every one starts with, the icons, then
    their group."""
    out = resource(0, 0, 0, b'')
    for at, png in enumerate(pngs):
        out += resource(RT_ICON, at + 1, ICON_FLAGS, png)
    return out + resource(RT_GROUP_ICON, 1, GROUP_FLAGS, group(pngs))


def read_ico(data):
    """The PNG payloads of an ICO file, after checking its fixed fields and
    that each entry's size and offset describe its payload."""
    reserved, kind, count = struct.unpack_from('<HHH', data, 0)
    if (reserved, kind, count) != (0, 1, len(SIZES)):
        raise ValueError(f'header {reserved, kind, count}')
    pngs, at = [], 6 + 16 * count
    for index, size in enumerate(SIZES):
        width, height, colors, zero, planes, bits, length, offset = struct.unpack_from(
            '<BBBBHHII', data, 6 + 16 * index)
        if (width, height, colors, zero, planes, bits) != (size % 256, size % 256, 0, 0, 1, 32):
            raise ValueError(f'entry {index}: {width, height, colors, zero, planes, bits}')
        if offset != at or offset + length > len(data):
            raise ValueError(f'entry {index}: offset {offset} for {length} bytes')
        pngs.append(data[offset:offset + length])
        at += length
    if at != len(data):
        raise ValueError(f'{len(data) - at} bytes after the last image')
    return pngs


def read_res(data):
    """The `RT_ICON` payloads of a `.res` file, after checking the fixed
    fields of every entry and that each size and byte count describes its
    payload."""
    entries, at = [], 0
    while at < len(data):
        size, head, ff1, kind, ff2, name, version, flags, language, _, _ = struct.unpack_from(
            '<IIHHHHIHHII', data, at)
        if (head, ff1, ff2, version) != (32, 0xFFFF, 0xFFFF, 0):
            raise ValueError(f'entry at {at}: not an ordinal header')
        body = data[at + 32:at + 32 + size]
        if len(body) != size:
            raise ValueError(f'entry at {at}: {size} bytes past the end')
        entries.append((kind, name, flags, language, body))
        at += 32 + size + (-size % 4)
    kinds = [(kind, name, flags, language) for kind, name, flags, language, _ in entries]
    want = [(0, 0, 0, 0)] + [(RT_ICON, at + 1, ICON_FLAGS, 0) for at in range(len(SIZES))]
    if kinds != want + [(RT_GROUP_ICON, 1, GROUP_FLAGS, 0)]:
        raise ValueError(f'entries {kinds}')
    if entries[0][4]:
        raise ValueError('the first entry is not empty')
    pngs = [body for *_, body in entries[1:-1]]
    if entries[-1][4] != group(pngs):
        raise ValueError('the group does not describe its icons')
    return pngs


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help='compare windows/ with a fresh render; write nothing')
    parser.add_argument('--out', type=pathlib.Path, default=WINDOWS,
                        help='the folder to write, windows/ by default')
    args = parser.parse_args()
    images = [(size, size, draw(size)) for size in SIZES]
    if args.check:
        failed = False
        for name, read in (('icon.ico', read_ico), ('icon.res', read_res)):
            path = WINDOWS / name
            try:
                pngs = read(path.read_bytes())
                decoded = [sprites.decode(png) for png in pngs]
            except (OSError, ValueError, zlib.error, struct.error) as error:
                print(f'{name}: unreadable: {error}', file=sys.stderr)
                failed = True
                continue
            if decoded == images:
                print(f'{name}: matches')
            else:
                print(f'{name}: differs from a fresh render', file=sys.stderr)
                failed = True
        return 1 if failed else 0
    pngs = [sprites.encode(*image) for image in images]
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / 'icon.ico').write_bytes(ico(pngs))
    (args.out / 'icon.res').write_bytes(res(pngs))
    print(f'wrote icon.ico and icon.res to {args.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
