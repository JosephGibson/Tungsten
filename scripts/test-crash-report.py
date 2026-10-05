#!/usr/bin/env python3
"""Synthetic tests for scripts/crash-report.py; fake symbolizers, no binutils, LLVM or cargo."""

import contextlib
import importlib.util
import io
import struct
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


crash = load("crash_report", "crash-report.py")
fixtures = load("test_release", "test-release.py")
BUILD_ID = fixtures.BUILD_ID
GUID = fixtures.GUID
LINUX_TARGET = "x86_64-unknown-linux-gnu"
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
PLAYER = "/home/player/Downloads/tungsten-examples-v0.48.0-x86_64-unknown-linux-gnu"

LINUX_CRASH = f"""\
Tungsten crash report
game: my-game
game_version: (none)
engine_version: 0.48.0
target: {LINUX_TARGET}
executable: {PLAYER}/bin/x86-64-v3/game
time: 20261005T120000.000Z
thread: main
message: boom
location: src/main.rs:10:5
anchor: 0x5600000a1000 tungsten::crash::anchor
build_id: {{build_id}}
backtrace:
   0:     0x5600000b0010 - std::backtrace_rs::backtrace::libunwind::trace
                               at /rustc/x/library/std/src/../../backtrace/src/backtrace/libunwind.rs:117:9
   1:     0x5600000b0010 - <std::backtrace::Backtrace>::create
   2:     0x5600000a2005 - game::inner
   3:     0x7f0000001234 - __libc_start_call_main
   4:     0x5600000a3007 - game::main
modules:
560000000000-5600000a0000 r--p 00000000 103:02 7                          {PLAYER}/bin/x86-64-v3/game
5600000a0000-5600000c0000 r-xp 000a0000 103:02 7                          {PLAYER}/bin/x86-64-v3/game
7f0000000000-7f0000100000 r-xp 00000000 103:02 9                          /usr/lib/libc.so.6
"""
ADDR2LINE = """\
0x00000000000b0010
<std::backtrace::Backtrace>::create
/rustc/x/library/std/src/backtrace.rs:331
0x00000000000a2004
game::helper
/build/src/helper.rs:5 (discriminator 3)
game::inner
/build/src/main.rs:20
0x00000000000a3006
game::main
/build/src/main.rs:30
"""

WINDOWS_CRASH = f"""\
Tungsten crash report
game: my-game
target: {WINDOWS_TARGET}
executable: C:\\Users\\player\\Games\\tungsten-examples-v0.48.0-{WINDOWS_TARGET}\\bin\\x86-64\\game.exe
thread: main
message: boom
anchor: 0x7ff612341010 tungsten::crash::anchor
build_id: {{build_id}} D:\\a\\release\\deps\\game.pdb
backtrace:
   0:     0x7ff612342000 - <unknown>
   1:     0x7ff612343005 - <unknown>
   2:     0x7ffa00001000 - <unknown>
"""
LLVM = """\
0x2000
std::backtrace::Backtrace::create
C:\\rustc\\library\\std\\src\\backtrace.rs:331
0x3004
game::main
D:\\a\\Tungsten\\src\\main.rs:30
"""


def pad4(data):
    return data + b"\0" * (-len(data) % 4)


def record(kind, body):
    body = pad4(body)
    return struct.pack("<HH", len(body) + 2, kind) + body


def public(name, segment, offset):
    return record(crash.S_PUB32, struct.pack("<IIH", 0, offset, segment) + name.encode() + b"\0")


def procedure(name, segment, offset, kind=0x110F):
    return record(kind, struct.pack("<IIIIIIIIHB", 0, 0, 0, 16, 0, 16, 0, offset, segment, 0) + name.encode() + b"\0")


def synthetic_msf(streams, block_size=512):
    """An MSF 7.0 file holding `streams`: superblock, free-page maps, block map, data, directory."""
    blocks, placed, at = {}, [], 4
    for data in streams:
        count = -(-len(data) // block_size)
        placed.append(list(range(at, at + count)))
        for n, index in enumerate(placed[-1]):
            blocks[index] = data[n * block_size:(n + 1) * block_size]
        at += count
    directory = struct.pack(f"<{1 + len(streams)}I", len(streams), *map(len, streams))
    directory += b"".join(struct.pack(f"<{len(p)}I", *p) for p in placed)
    count = -(-len(directory) // block_size)
    for n in range(count):
        blocks[at + n] = directory[n * block_size:(n + 1) * block_size]
    total = at + count
    blocks[0] = crash.release.MSF_MAGIC + struct.pack("<6I", block_size, 1, total, len(directory), 0, 3)
    blocks[3] = struct.pack(f"<{count}I", *range(at, total))
    return b"".join(blocks.get(n, b"").ljust(block_size, b"\0") for n in range(total))


def synthetic_symbol_pdb(guid, age, publics=b"", module=b""):
    """A PDB with an info stream, a DBI stream naming the symbol records and one module."""
    info = struct.pack("<III", 20000404, 0, age) + guid
    symbols_size = 4 + len(module)
    modinfo = pad4(struct.pack("<I28xHHIIIH2xIII", 0, 0, 5, symbols_size, 0, 0, 0, 0, 0, 0) + b"mod.o\0obj.o\0")
    dbi = bytearray(64) + modinfo
    struct.pack_into("<H", dbi, 20, 4)
    struct.pack_into("<i", dbi, 24, len(modinfo))
    return synthetic_msf([b"", info, b"", bytes(dbi), publics, struct.pack("<I", 4) + module])


def windows_pe(guid, age, image_size=0x10000):
    pe = bytearray(fixtures.synthetic_pe(guid, age, r"D:\a\release\deps\game.pdb"))
    struct.pack_into("<I", pe, 0x40 + 24 + 56, image_size)
    return bytes(pe)


class Fake:
    """Stands in for subprocess.run: records each command and returns `stdout`."""

    def __init__(self, stdout):
        self.stdout, self.calls = stdout, []

    def __call__(self, command, **_kwargs):
        self.calls.append(command)
        return SimpleNamespace(returncode=0, stdout=self.stdout, stderr="")


class CrashReport(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def write(self, path, data):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(data, bytes):
            target.write_bytes(data)
        else:
            target.write_text(data)
        return target

    def linux_archive(self, build_id=BUILD_ID, crash_id=None):
        """A player archive with its debug archive over it, and a crash file from elsewhere."""
        self.write("archive/bin/x86-64-v3/game", b"binary")
        self.write("archive/bin/x86-64-v3/game.debug", fixtures.synthetic_elf(build_id, [".debug_line"]))
        claimed = BUILD_ID.hex() if crash_id is None else crash_id
        return self.write("crash.txt", LINUX_CRASH.format(build_id=claimed))

    def symbolize(self, *args, run=None, **kwargs):
        out = io.StringIO()
        code = crash.symbolize(*args, run=run or Fake(ADDR2LINE), out=out, **kwargs)
        return code, out.getvalue()

    def test_a_linux_crash_file_parses_with_module_bases_and_the_minus_one_rule(self):
        report = crash.parse(LINUX_CRASH.format(build_id="ab"))
        self.assertEqual(report.fields["game"], "my-game")
        self.assertEqual(report.fields["build_id"], "ab")
        self.assertEqual([(f.index, f.returns) for f in report.frames],
                         [(0, False), (1, False), (2, True), (3, True), (4, True)])
        game = f"{PLAYER}/bin/x86-64-v3/game"
        self.assertEqual(crash.module_bases(report.modules),
                         {game: 0x560000000000, "/usr/lib/libc.so.6": 0x7F0000000000})
        self.assertEqual(crash.executable_module(report), game)
        # The instruction pointer's frame keeps its address; return addresses lose one.
        self.assertEqual(crash.linux_offsets(report, game), {0: 0xB0010, 1: 0xB0010, 2: 0xA2004, 4: 0xA3006})

    def test_a_fake_addr2line_runs_once_per_module_and_its_output_is_parsed(self):
        path = self.linux_archive()
        fake = Fake(ADDR2LINE)
        code, out = self.symbolize(path, root=self.root / "archive", run=fake)
        self.assertEqual(code, 0, out)
        debug = self.root / "archive/bin/x86-64-v3/game.debug"
        self.assertEqual(fake.calls, [["addr2line", "-a", "-C", "-f", "-i", "-e", str(debug),
                                       "0xb0010", "0xa2004", "0xa3006"]])
        self.assertIn(f"build_id {BUILD_ID.hex()} matches {debug}", out)
        self.assertEqual(out.splitlines()[1:], [
            "0: <std::backtrace::Backtrace>::create at /rustc/x/library/std/src/backtrace.rs:331",
            "2: game::helper at /build/src/helper.rs:5",
            "2: game::inner at /build/src/main.rs:20",
            "4: game::main at /build/src/main.rs:30",
        ])
        self.assertEqual(crash.parse_symbolizer("0x10\n??\n??:0\n"), {0x10: [("??", "??:0")]})

    def test_root_maps_another_machines_executable_to_the_binary_and_debug_file(self):
        report = crash.parse(LINUX_CRASH.format(build_id="ab"))
        self.write("archive/bin/x86-64-v3/game", b"binary")
        root = self.root / "archive"
        self.assertEqual(crash.from_root(report, root, False),
                         (root / "bin/x86-64-v3/game", root / "bin/x86-64-v3/game.debug"))
        windows = crash.parse(WINDOWS_CRASH.format(build_id="00"))
        self.write("archive/bin/x86-64/game.exe", windows_pe(GUID, 1))
        self.assertEqual(crash.from_root(windows, root, True),
                         (root / "bin/x86-64/game.exe", root / "bin/x86-64/game.pdb"))
        report.fields["executable"] = "/opt/game/game"
        with self.assertRaisesRegex(crash.CrashReportError, "no bin/<level>/<name> tail"):
            crash.from_root(report, root, False)
        report.fields["executable"] = f"{PLAYER}/bin/x86-64/game"
        with self.assertRaisesRegex(crash.CrashReportError, "no such binary"):
            crash.from_root(report, root, False)

    def test_a_build_id_that_differs_or_is_missing_exits_1_before_resolving(self):
        for crash_id, message in ((bytes(20).hex(), "is build 000102.*the crash file's is 0000"),
                                  ("(none)", "records no build_id"),
                                  ("", "records no build_id")):
            with self.subTest(crash_id=crash_id):
                path = self.linux_archive(crash_id=crash_id)
                fake = Fake(ADDR2LINE)
                with self.assertRaisesRegex(crash.CrashReportError, message):
                    self.symbolize(path, root=self.root / "archive", run=fake)
                self.assertEqual(fake.calls, [])
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            path = self.linux_archive()
            self.assertEqual(crash.main(["symbolize", str(path), "--root", str(self.root / "archive"),
                                         "--debug", str(path)]), 1)
            path = self.linux_archive(crash_id=bytes(20).hex())
            self.assertEqual(crash.main(["symbolize", str(path), "--root", str(self.root / "archive")]), 1)
        self.assertIn("ERROR: give either --root or --binary/--debug, not both", stderr.getvalue())
        self.assertIn("ERROR: ", stderr.getvalue().splitlines()[1])

    def test_the_windows_anchor_arithmetic(self):
        report = crash.parse(WINDOWS_CRASH.format(build_id="00"))
        # Base = anchor 0x7ff612341010 less its RVA 0x1010; the third frame is outside the image.
        self.assertEqual(crash.windows_rvas(report, 0x1010, 0x10000), {0: 0x2000, 1: 0x3004})
        pdb = synthetic_symbol_pdb(GUID, 1, publics=public("_ZN4core3fmt5write17h0E", 1, 0x400)
                                   + public("_RNvNtCs1a_8tungsten5crash6anchor", 1, 0x10))
        self.assertEqual(crash.pdb_anchor(pdb), (1, 0x10))
        self.assertEqual(crash.anchor_rva(pdb, windows_pe(GUID, 1)), 0x1010)
        # Without a public symbol, the procedure record of the module that defines it.
        pdb = synthetic_symbol_pdb(GUID, 1, module=procedure("tungsten::crash::anchor", 1, 0x10))
        self.assertEqual(crash.pdb_anchor(pdb), (1, 0x10))
        with self.assertRaisesRegex(crash.CrashReportError, "no tungsten::crash::anchor symbol"):
            crash.pdb_anchor(synthetic_symbol_pdb(GUID, 1, module=procedure("game::main", 1, 0)))
        self.assertEqual(crash.pdb_identity(GUID, 1), "00112233445566778899AABBCCDDEEFF1")

    def test_windows_resolves_rvas_beside_its_binary_and_refuses_to_run_without_it(self):
        identity = crash.pdb_identity(GUID, 1)
        self.write("archive/bin/x86-64/game.exe", windows_pe(GUID, 1))
        self.write("archive/bin/x86-64/game.pdb", synthetic_symbol_pdb(
            GUID, 1, publics=public("_RNvNtCs1a_8tungsten5crash6anchor", 1, 0x10)))
        path = self.write("crash.txt", WINDOWS_CRASH.format(build_id=identity))
        fake = Fake(LLVM)
        with mock.patch.object(crash, "llvm_symbolizer", return_value="llvm-symbolizer"):
            code, out = self.symbolize(path, root=self.root / "archive", run=fake, expect="src/main.rs")
        self.assertEqual(code, 0, out)
        command = fake.calls[0]
        self.assertEqual(command[0], "llvm-symbolizer")
        self.assertTrue(command[1].startswith("--obj=") and command[1].endswith("game.exe"), command)
        self.assertEqual(command[-2:], ["0x2000", "0x3004"])
        self.assertIn("--relative-address", command)
        self.assertIn("1: game::main at D:\\a\\Tungsten\\src\\main.rs:30", out)
        with self.assertRaisesRegex(crash.CrashReportError, "needs the binary"):
            self.symbolize(path, debug=self.root / "archive/bin/x86-64/game.pdb")
        # A PDB of another build is refused before any symbolizer runs.
        self.write("archive/bin/x86-64/game.pdb", synthetic_symbol_pdb(bytes(16), 1))
        fake = Fake(LLVM)
        with self.assertRaisesRegex(crash.CrashReportError, "is build 0000"):
            self.symbolize(path, root=self.root / "archive", run=fake)
        self.assertEqual(fake.calls, [])

    def test_expect_matching_and_missing(self):
        path = self.linux_archive()
        code, out = self.symbolize(path, root=self.root / "archive", expect="build/src/main.rs")
        self.assertEqual(code, 0, out)
        code, out = self.symbolize(path, root=self.root / "archive", expect="tools/crash-probe/src/main.rs")
        self.assertEqual(code, 1)
        self.assertIn("ERROR: no resolved frame is in tools/crash-probe/src/main.rs", out)
        code, out = self.symbolize(path, root=self.root / "archive", run=Fake("0xb0010\n??\n??:0\n"))
        self.assertEqual(code, 1)
        self.assertIn("ERROR: no frame of the executable resolved", out)


if __name__ == "__main__":
    unittest.main()
