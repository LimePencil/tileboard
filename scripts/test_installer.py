#!/usr/bin/env python3
"""Exercise the piped POSIX installer with isolated release and platform fixtures."""

import hashlib
import io
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

INSTALLER = Path(__file__).resolve().parents[1] / "install.sh"
# Mock network/platform boundaries; use real extraction, hashing, and shell execution.
SHIM = r'''
import os, pathlib, shutil, sys
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
root = pathlib.Path(os.environ["TB_FIXTURE"])
if name == "uname":
    print(os.environ.get("TB_OS", "Linux") if args == ["-s"] else os.environ.get("TB_ARCH", "x86_64"))
elif name == "getconf":
    print(os.environ.get("TB_LIBC", "glibc 2.35"))
elif name == "sysctl":
    print(os.environ.get("TB_APPLE_ARM", "0"))
elif name == "cygpath":
    print("C:\\Users\\Test\\bin")
elif name == "powershell.exe":
    (root / "windows-path").write_text(os.environ["TILEBOARD_WINDOWS_INSTALL_DIR"])
elif name == "curl":
    if os.environ.get("TB_NETWORK_FAIL"):
        sys.exit(22)
    if "--head" in args:
        print("https://github.com/LimePencil/tileboard/releases/tag/v0.1.0", end="")
    else:
        name = args[-1].rsplit("/", 1)[-1]
        with (root / "downloads").open("a") as log:
            log.write(name + "\n")
        source = root / name
        if not source.exists():
            sys.exit(22)
        shutil.copyfile(source, args[args.index("--output") + 1])
'''


@unittest.skipIf(os.name == "nt", "Fixture executable scripts require a POSIX host")
class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="tileboard-installer-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "installed bin"
        self.profile = self.root / "shell profile"
        self.mock = self.root / "commands"
        self.mock.mkdir()
        for name in ("uname", "getconf", "sysctl", "curl", "cygpath", "powershell.exe"):
            command = self.mock / name
            # exec a quoted absolute interpreter; support Python installations with spaces.
            command.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " "
                               + shlex.quote(str(self.mock / "shim.py"))
                               + " " + shlex.quote(name) + ' "$@"\n')
            command.chmod(0o755)
        # Pass the command name through argv because the shared shim has its own filename.
        (self.mock / "shim.py").write_text(SHIM.replace(
            'name = pathlib.Path(sys.argv[0]).name\nargs = sys.argv[1:]',
            'name = sys.argv[1]\nargs = sys.argv[2:]'))
        self.env = dict(os.environ, PATH=str(self.mock) + os.pathsep + os.environ["PATH"],
                        TB_FIXTURE=str(self.root), SHELL="/bin/bash", TMPDIR=str(self.root))
        self.manifest = []
        for target in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
                       "x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-pc-windows-msvc"):
            self.archive(target)

    def archive(self, target, body=b'#!/bin/sh\necho "tileboard 0.1.0"\n'):
        root = "tileboard-v0.1.0-" + target
        windows = "windows" in target
        filename = root + (".zip" if windows else ".tar.gz")
        path = self.root / filename
        member = root + ("/tileboard.exe" if windows else "/tileboard")
        if windows:
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr(member, body)
        else:
            with tarfile.open(path, "w:gz") as archive:
                info = tarfile.TarInfo(member)
                info.size = len(body)
                info.mode = 0o755
                archive.addfile(info, io.BytesIO(body))
        self.manifest = [line for line in self.manifest if filename not in line]
        self.manifest.append(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + filename)
        (self.root / "SHA256SUMS").write_text("\n".join(self.manifest) + "\n")
        return path

    def install(self, *args, success=True, **env):
        result = subprocess.run(["sh", "-s", "--", "--bin-dir", str(self.bin),
                                 "--profile", str(self.profile), *args],
                                input=INSTALLER.read_text(), text=True,
                                capture_output=True, env=dict(self.env, **env))
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse(list(self.root.glob("tileboard-install.*")))
        self.assertFalse(list(self.bin.glob(".tileboard.*")))
        return result

    def test_platforms_and_rosetta(self):
        cases = [("Linux", "x86_64", "0", "x86_64-unknown-linux-gnu"),
                 ("Linux", "aarch64", "0", "aarch64-unknown-linux-gnu"),
                 ("Darwin", "x86_64", "0", "x86_64-apple-darwin"),
                 ("Darwin", "arm64", "1", "aarch64-apple-darwin"),
                 ("Darwin", "x86_64", "1", "aarch64-apple-darwin"),
                 ("MINGW64_NT-10.0", "x86_64", "0", "x86_64-pc-windows-msvc"),
                 ("MSYS_NT-10.0", "x86_64", "0", "x86_64-pc-windows-msvc"),
                 ("CYGWIN_NT-10.0", "x86_64", "0", "x86_64-pc-windows-msvc")]
        for os_name, arch, apple_arm, target in cases:
            with self.subTest(target=target, os=os_name, arch=arch):
                self.install(TB_OS=os_name, TB_ARCH=arch, TB_APPLE_ARM=apple_arm)
                self.assertIn(target, (self.root / "downloads").read_text().splitlines()[-2])
                binary = self.bin / ("tileboard.exe" if "windows" in target else "tileboard")
                self.assertEqual(subprocess.check_output([str(binary), "--version"], text=True).strip(),
                                 "tileboard 0.1.0")
        self.assertEqual((self.root / "windows-path").read_text(), "C:\\Users\\Test\\bin")

    def test_path_is_literal_and_idempotent(self):
        self.bin = self.root / "bin '$`touch SHOULD_NOT_EXIST` $(false) [*] \\ end"
        self.profile.write_text("# existing content without final newline")
        self.install("--version", "0.1.0")
        original = self.profile.read_text()
        self.install("--version", "v0.1.0")
        self.assertEqual(self.profile.read_text(), original)
        for shell in ("sh", "bash", "zsh"):
            if not shutil.which(shell):
                continue
            result = subprocess.run([shell, "-c", '. "$1"; . "$1"; printf "%s" "$PATH"',
                                     "test", str(self.profile)], capture_output=True, text=True,
                                    env=self.env, cwd=self.root, check=True)
            self.assertEqual(result.stdout.split(":"), [str(self.bin), *self.env["PATH"].split(":")])
        self.assertFalse((self.root / "SHOULD_NOT_EXIST").exists())

    @unittest.skipUnless(shutil.which("fish"), "fish is optional locally; installed in Linux CI")
    def test_fish_path(self):
        self.bin = self.root / "fish ' \\ $() [*] bin"
        self.install("--shell", "fish")
        original = self.profile.read_text()
        self.install("--shell", "fish")
        self.assertEqual(original, self.profile.read_text())
        result = subprocess.run(["fish", "--no-config", "-c",
                                 'source $argv[1]; source $argv[1]; printf "%s\\n" $PATH',
                                 str(self.profile)], env=self.env, capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.splitlines(), [str(self.bin), *self.env["PATH"].split(":")])

    def test_no_path_modification(self):
        self.install("--no-modify-path", TB_OS="MINGW64_NT-10.0")
        self.assertFalse(self.profile.exists())
        self.assertFalse((self.root / "windows-path").exists())

    def test_failures_preserve_existing_install(self):
        self.bin.mkdir()
        old = self.bin / "tileboard"
        old.write_text("old installation")
        cases = [dict(TB_OS="FreeBSD"), dict(TB_ARCH="i686"), dict(TB_LIBC="musl"),
                 dict(TB_LIBC="glibc 2.34"), dict(TB_NETWORK_FAIL="1"),
                 dict(TB_OS="MINGW64_NT", TB_ARCH="arm64")]
        for env in cases:
            with self.subTest(env=env):
                self.install(success=False, **env)
                self.assertEqual(old.read_text(), "old installation")
                self.assertFalse(self.profile.exists())
        manifest = self.root / "SHA256SUMS"
        for contents in ("", "\n".join(self.manifest * 2),
                         "\n".join("0" * 64 + line[64:] for line in self.manifest)):
            manifest.write_text(contents)
            self.install(success=False)
            self.assertEqual(old.read_text(), "old installation")
            self.assertFalse(self.profile.exists())
        self.archive("x86_64-unknown-linux-gnu", b"#!/bin/sh\nexit 1\n")
        self.install(success=False)
        self.assertEqual(old.read_text(), "old installation")
        self.assertFalse(self.profile.exists())

    def test_bad_archive_after_valid_checksum(self):
        path = self.archive("x86_64-unknown-linux-gnu")
        path.write_bytes(b"not an archive")
        (self.root / "SHA256SUMS").write_text(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.name)
        self.install(success=False)
        self.assertFalse(self.bin.exists())
        self.assertFalse(self.profile.exists())

    def test_options_and_unset_shell(self):
        for args in [("--version", "../../evil"), ("--wat",), ("--version",),
                     ("--bin-dir", "relative"), ("--bin-dir", "/a:b"),
                     ("--profile", "/a\nb"), ("--shell", "unknown")]:
            with self.subTest(args=args):
                self.install(*args, success=False)
                self.assertFalse((self.root / "downloads").exists())
                self.assertFalse(self.bin.exists())
        del self.env["SHELL"]
        self.install()


if __name__ == "__main__":
    unittest.main(verbosity=2)
