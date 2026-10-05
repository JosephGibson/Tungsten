#!/usr/bin/env python3
"""Symbolize Tungsten crash reports (stdlib, with binutils addr2line on Linux and
llvm-symbolizer on Windows).

  symbolize CRASH (--root DIR | [--binary FILE] --debug FILE) [--expect FRAGMENT]
      Checks CRASH's build_id against the debug file first, the GNU build-id on Linux
      and the PDB's GUID and age on Windows, and exits 1 on a mismatch or a missing
      identity before resolving anything. Then prints `N: function at file:line` for the
      executable's frames. --root is a folder holding a player archive with its debug
      archive extracted over it: only the bin/<level>/<name> tail of CRASH's executable
      path, a path on the crashing machine, picks the binary and its debug file. Windows
      needs the binary (--root or --binary). Exits 1 when no frame of the executable
      resolves, or none matches --expect.
  probe RELEASE_DIR --target TRIPLE
      Copies tools/crash-probe's build, and on Windows its PDB, from RELEASE_DIR into an
      archive-like folder away from the build paths, splits the Linux copy as release.py
      package does, runs it with TUNGSTEN_USER_DIR in another temporary folder and
      symbolizes its crash report with --root and --expect tools/crash-probe/src/main.rs.

Linux: a frame's module base is the start of its file's offset-0 mapping, from the
report's modules list. Windows: the base is the anchor's runtime address less its RVA,
which the PDB's symbols and the binary's section table give. Every frame but the first
is a return address, so its address less one is resolved.

Rationale: D-119, D-120. Release steps: docs/releases.md.
"""

import argparse
import importlib.util
import itertools
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from collections import namedtuple
from pathlib import Path

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

PROBE = "tungsten-crash-probe"
PROBE_SOURCE = "tools/crash-probe/src/main.rs"
FRAME_RE = re.compile(r"^\s*(\d+):\s+(0x[0-9a-fA-F]+)\b")
MAPPING_RE = re.compile(r"^([0-9a-fA-F]+)-([0-9a-fA-F]+)\s+\S+\s+([0-9a-fA-F]+)\s+\S+\s+\d+\s+(.+)$")
TAIL_RE = re.compile(r"(?:^|[\\/])bin[\\/]([^\\/]+)[\\/]([^\\/]+)$")
ADDRESS_RE = re.compile(r"^0x[0-9a-fA-F]+$")
DISCRIMINATOR_RE = re.compile(r" \(discriminator \d+\)$")
# The anchor's names: legacy and v0 mangling, and CodeView's qualified name.
ANCHOR_RE = re.compile(r"8tungsten5crash6anchor|tungsten::crash::anchor")
S_PUB32, S_LPROC32, S_GPROC32, S_LPROC32_ID, S_GPROC32_ID = 0x110E, 0x110F, 0x1110, 0x1146, 0x1147

Report = namedtuple("Report", "fields frames modules")
Frame = namedtuple("Frame", "index address returns")
Mapping = namedtuple("Mapping", "start end offset path")


class CrashReportError(Exception):
    pass


def parse(text):
    """The report's `key: value` fields, its frames and its modules list."""
    fields, raw, modules, part = {}, [], [], "header"
    for line in text.splitlines():
        if line in ("backtrace:", "modules:"):
            part = line[:-1]
        elif part == "header":
            key, sep, value = line.partition(": ")
            if sep:
                fields.setdefault(key, value)
        elif part == "backtrace":
            match = FRAME_RE.match(line)
            if match:
                raw.append((int(match[1]), int(match[2], 16)))
        else:
            match = MAPPING_RE.match(line)
            if match:
                start, end, offset = (int(match[n], 16) for n in (1, 2, 3))
                modules.append(Mapping(start, end, offset, match[4].strip()))
    # The leading lines that share the first address are the one frame that holds the
    # instruction pointer itself (std prints each inlined symbol on its own line).
    frames, leading = [], True
    for index, address in raw:
        leading = leading and address == raw[0][1]
        frames.append(Frame(index, address, not leading))
    return Report(fields, frames, modules)


def module_bases(modules):
    """{path: base}: the start of each file's offset-0 mapping."""
    bases = {}
    for mapping in modules:
        if mapping.offset == 0:
            bases[mapping.path] = min(mapping.start, bases.get(mapping.path, mapping.start))
    return bases


def module_of(address, modules):
    return next((m.path for m in modules if m.start <= address < m.end), None)


def executable_module(report):
    """The modules-list path of the crashing executable."""
    executable = report.fields.get("executable", "")
    paths = {m.path for m in report.modules}
    if executable in paths:
        return executable
    name = re.split(r"[\\/]", executable)[-1]
    return next((p for p in sorted(paths) if p.rsplit("/", 1)[-1] == name), None)


def linux_offsets(report, module):
    """{frame index: file address} for the frames in `module`'s mappings."""
    base = module_bases(report.modules).get(module)
    if base is None:
        return {}
    return {frame.index: frame.address - base - frame.returns for frame in report.frames
            if module_of(frame.address, report.modules) == module}


def windows_rvas(report, anchor_rva, image_size):
    """{frame index: RVA} for the frames in the image: address less anchor plus anchor RVA."""
    if "anchor" not in report.fields:
        raise CrashReportError("the crash file has no anchor line, so the image base is unknown")
    anchor = int(report.fields["anchor"].split()[0], 16)
    base = anchor - anchor_rva
    return {frame.index: frame.address - base - frame.returns for frame in report.frames
            if base <= frame.address < base + image_size}


def parse_symbolizer(output):
    """{address: [(function, location)]} from `addr2line -a -f -i` or llvm-symbolizer's
    GNU style with --print-address: an address line, then function and location pairs."""
    groups, current, lines = {}, None, [line.strip() for line in output.splitlines()]
    at = 0
    while at < len(lines):
        if ADDRESS_RE.match(lines[at]):
            current = groups.setdefault(int(lines[at], 16), [])
            at += 1
        elif current is not None and lines[at] and at + 1 < len(lines):
            current.append((lines[at], DISCRIMINATOR_RE.sub("", lines[at + 1])))
            at += 2
        else:
            at += 1
    return groups


def resolved(pairs):
    return [(function, location) for function, location in pairs
            if function != "??" and not location.startswith("??")]


def unique_hex(addresses):
    """Each address once, in order, as the symbolizer's arguments."""
    return [hex(address) for address in dict.fromkeys(addresses.values())]


def run_symbolizer(command, run):
    result = run(command, capture_output=True, text=True)
    if result.returncode != 0:
        raise CrashReportError(f"{command[0]} failed: {result.stderr.strip()}")
    return parse_symbolizer(result.stdout)


def pdb_identity(guid, age):
    """GUID and age in symbol-server form, as crash files print them."""
    data1, data2, data3 = struct.unpack_from("<IHH", guid)
    return f"{data1:08X}{data2:04X}{data3:04X}{guid[8:].hex().upper()}{age:X}"


def check_identity(report, debug, windows):
    """The crash file's build_id against the debug file's, before anything is resolved."""
    claimed = report.fields.get("build_id", "").split(" ", 1)[0]
    if not claimed or claimed == "(none)":
        raise CrashReportError("the crash file records no build_id, so no debug file can be trusted")
    data = Path(debug).read_bytes()
    if windows:
        actual = pdb_identity(*release.pdb_info(data))
    else:
        build_id = release.elf_build_id(data)
        actual = build_id.hex() if build_id else "(none)"
    if actual.lower() != claimed.lower():
        raise CrashReportError(f"{debug} is build {actual}, the crash file's is {claimed}")
    return actual


def from_root(report, root, windows):
    """The binary and debug file under ROOT that the executable's bin/<level>/<name> tail names."""
    match = TAIL_RE.search(report.fields.get("executable", ""))
    if not match:
        raise CrashReportError(f"executable {report.fields.get('executable')!r} has no bin/<level>/<name> tail")
    level, name = match[1], match[2]
    binary = Path(root) / "bin" / level / name
    if not binary.is_file():
        raise CrashReportError(f"{binary}: no such binary under --root")
    if windows:
        record = release.codeview(binary.read_bytes())
        if record is None:
            raise CrashReportError(f"{binary}: no CodeView record naming a PDB")
        return binary, binary.parent / re.split(r"[\\/]", record.name)[-1]
    return binary, binary.parent / f"{name}.debug"


def pe_layout(data):
    """(section virtual addresses, SizeOfImage) of a PE file."""
    try:
        (pe,) = struct.unpack_from("<I", data, 0x3C)
        if data[pe:pe + 4] != b"PE\0\0":
            raise CrashReportError("not a PE file")
        sections, optional_size = struct.unpack_from("<H", data, pe + 6)[0], struct.unpack_from("<H", data, pe + 20)[0]
        optional = pe + 24
        (image_size,) = struct.unpack_from("<I", data, optional + 56)
        table = optional + optional_size
        addresses = [struct.unpack_from("<I", data, table + 40 * n + 12)[0] for n in range(sections)]
        return addresses, image_size
    except struct.error as exc:
        raise CrashReportError(f"malformed PE file: {exc}") from None


def msf_streams(data):
    """Every stream of an MSF 7.0 file."""
    if data[:len(release.MSF_MAGIC)] != release.MSF_MAGIC:
        raise CrashReportError("not an MSF 7.0 PDB file")
    block_size, _fpm, _blocks, directory_size, _unknown, block_map = struct.unpack_from("<6I", data, 32)

    def stream(blocks, size):
        return b"".join(data[n * block_size:(n + 1) * block_size] for n in blocks)[:size]

    count = -(-directory_size // block_size)
    directory = stream(struct.unpack_from(f"<{count}I", data, block_map * block_size), directory_size)
    (total,) = struct.unpack_from("<I", directory, 0)
    sizes = [0 if size == 0xFFFFFFFF else size for size in struct.unpack_from(f"<{total}I", directory, 4)]
    streams, at = [], 4 + 4 * total
    for size in sizes:
        count = -(-size // block_size)
        streams.append(stream(struct.unpack_from(f"<{count}I", directory, at), size))
        at += 4 * count
    return streams


def symbol_records(data, start=0, end=None):
    """(kind, body) for each CodeView symbol record in data[start:end]."""
    end = len(data) if end is None else end
    while start + 4 <= end:
        length, kind = struct.unpack_from("<HH", data, start)
        yield kind, data[start + 4:start + 2 + length]
        start += 2 + max(length, 2)


def cstring(body, at):
    end = body.find(b"\0", at)
    return body[at:end if end >= 0 else len(body)].decode("utf-8", "replace")


def pdb_anchor(data):
    """(segment, offset) of tungsten::crash::anchor: its public symbol, else the procedure
    record in the module that defines it."""
    try:
        streams = msf_streams(data)
        dbi = streams[3]
        (records,) = struct.unpack_from("<H", dbi, 20)
        (modules_size,) = struct.unpack_from("<i", dbi, 24)
        for kind, body in symbol_records(streams[records]):
            if kind == S_PUB32 and ANCHOR_RE.search(cstring(body, 10)):
                offset, segment = struct.unpack_from("<IH", body, 4)
                return segment, offset
        at = 64
        while at < 64 + modules_size:
            module_stream, symbols_size = struct.unpack_from("<HI", dbi, at + 34)
            names = dbi.index(b"\0", dbi.index(b"\0", at + 64) + 1) + 1
            at = names + -names % 4
            if module_stream == 0xFFFF:
                continue
            for kind, body in symbol_records(streams[module_stream], 4, symbols_size):
                if kind in (S_LPROC32, S_GPROC32, S_LPROC32_ID, S_GPROC32_ID) and ANCHOR_RE.search(cstring(body, 35)):
                    offset, segment = struct.unpack_from("<IH", body, 28)
                    return segment, offset
    except (struct.error, IndexError, ValueError) as exc:
        raise CrashReportError(f"malformed PDB file: {exc}") from None
    raise CrashReportError("the PDB has no tungsten::crash::anchor symbol")


def anchor_rva(pdb_data, pe_data):
    segment, offset = pdb_anchor(pdb_data)
    addresses, _ = pe_layout(pe_data)
    if not 1 <= segment <= len(addresses):
        raise CrashReportError(f"anchor segment {segment} is not a section of the binary")
    return addresses[segment - 1] + offset


def llvm_symbolizer():
    found = shutil.which("llvm-symbolizer")
    if found:
        return found
    fallback = Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "LLVM" / "bin" / "llvm-symbolizer.exe"
    if fallback.is_file():
        return str(fallback)
    raise CrashReportError("llvm-symbolizer not found: install LLVM or put it on PATH")


def symbolize(crash, root=None, binary=None, debug=None, expect=None, run=subprocess.run, out=sys.stdout):
    """Prints the executable's resolved frames; returns the exit code."""
    report = parse(Path(crash).read_text(encoding="utf-8", errors="replace"))
    windows = "windows" in report.fields.get("target", "")
    if root is not None:
        binary, debug = from_root(report, root, windows)
    if debug is None:
        raise CrashReportError("give --root, or --debug with the binary's debug file")
    if windows and binary is None:
        raise CrashReportError("Windows symbolization needs the binary: give --root or --binary")
    identity = check_identity(report, debug, windows)
    print(f"build_id {identity} matches {debug}", file=out)
    if windows:
        rvas = windows_rvas(report, anchor_rva(Path(debug).read_bytes(), Path(binary).read_bytes()),
                            pe_layout(Path(binary).read_bytes())[1])
        with tempfile.TemporaryDirectory() as tmp:
            # llvm-symbolizer finds the PDB beside the binary under the record's name.
            local = Path(tmp) / Path(binary).name
            shutil.copyfile(binary, local)
            shutil.copyfile(debug, Path(tmp) / Path(debug).name)
            command = [llvm_symbolizer(), f"--obj={local}", "--relative-address", "--output-style=GNU",
                       "--print-address", "--inlines", "--demangle"]
            symbols = run_symbolizer(command + unique_hex(rvas), run) if rvas else {}
        addresses = rvas
    else:
        module = executable_module(report)
        addresses = linux_offsets(report, module) if module else {}
        # One call per module that has a symbol file: the executable, with its debug file.
        command = ["addr2line", "-a", "-C", "-f", "-i", "-e", str(debug)]
        symbols = run_symbolizer(command + unique_hex(addresses), run) if addresses else {}
    lines = []
    for _, group in itertools.groupby(report.frames, key=lambda frame: frame.address):
        # std prints each inlined symbol on its own line at one address: resolve the address
        # once and print its whole inline chain under the first line's index.
        first = next(group)
        lines += [(first.index, function, location)
                  for function, location in resolved(symbols.get(addresses.get(first.index), []))]
    for index, function, location in lines:
        print(f"{index}: {function} at {location}", file=out)
    if not lines:
        print("ERROR: no frame of the executable resolved", file=out)
        return 1
    if expect and not any(expect in location.replace("\\", "/") for _, _, location in lines):
        print(f"ERROR: no resolved frame is in {expect}", file=out)
        return 1
    return 0


def probe(release_dir, target, run=subprocess.run, out=sys.stdout):
    """Runs the probe from an archive-like copy and symbolizes its report; returns the exit code."""
    windows = "windows" in target
    built = Path(release_dir) / (PROBE + (".exe" if windows else ""))
    if not built.is_file():
        raise CrashReportError(f"{built}: build it first (cargo build --release -p {PROBE})")
    with tempfile.TemporaryDirectory() as tmp:
        root, user = Path(tmp) / "archive", Path(tmp) / "user"
        bin_dir = root / "bin" / release.BASELINE
        bin_dir.mkdir(parents=True)
        binary = bin_dir / built.name
        shutil.copyfile(built, binary)
        binary.chmod(0o755)
        if windows:
            pdb, name = release.matching_pdb(built, built.parent)
            shutil.copyfile(pdb, bin_dir / name)
        else:
            release.objcopy_split(binary, bin_dir / f"{PROBE}.debug")
        env = {key: value for key, value in os.environ.items()
               if key not in ("RUST_LOG", "TUNGSTEN_SMOKE_FRAMES", "TUNGSTEN_TEST_PANIC")}
        env["TUNGSTEN_USER_DIR"] = str(user)
        result = run([str(binary)], cwd=root, env=env, capture_output=True, text=True)
        print(f"{PROBE} exited {result.returncode}", file=out)
        crashes = sorted((user / "logs").glob("*-crash.txt"))
        if len(crashes) != 1:
            raise CrashReportError(f"expected one crash file under {user / 'logs'}, found {len(crashes)}: "
                                   f"{result.stderr.strip()[-500:]}")
        print(f"crash file {crashes[0].name}", file=out)
        return symbolize(crashes[0], root=root, expect=PROBE_SOURCE, run=run, out=out)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("symbolize", help="resolve a crash file's frames")
    command.add_argument("crash", type=Path)
    command.add_argument("--root", type=Path, help="a player archive with its debug archive extracted over it")
    command.add_argument("--binary", type=Path)
    command.add_argument("--debug", type=Path, help="the .debug file (Linux) or the PDB (Windows)")
    command.add_argument("--expect", help="fail unless a resolved frame's file contains this")
    command = commands.add_parser("probe", help="run tools/crash-probe from an archive-like copy")
    command.add_argument("release_dir", type=Path)
    command.add_argument("--target", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "symbolize":
            if args.root is not None and (args.binary is not None or args.debug is not None):
                raise CrashReportError("give either --root or --binary/--debug, not both")
            return symbolize(args.crash, args.root, args.binary, args.debug, args.expect)
        return probe(args.release_dir, args.target)
    except (CrashReportError, OSError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
