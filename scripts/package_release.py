#!/usr/bin/env python3
"""Verify and archive a native release binary. Requires Python 3.11+."""
import argparse
import pathlib
import shutil
import subprocess
import tempfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGETS = {
    'x86_64-unknown-linux-gnu',
    'aarch64-unknown-linux-gnu',
    'x86_64-apple-darwin',
    'aarch64-apple-darwin',
    'x86_64-pc-windows-msvc',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--target', choices=sorted(TARGETS))
    parser.add_argument('--check', action='store_true', help='Only validate the release version')
    args = parser.parse_args()
    package = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']
    version = package['version']
    if args.tag != f'v{version}':
        parser.error(f'Tag must match Cargo.toml: expected v{version}, received {args.tag!r}')
    locked = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))['package']
    if not any(p['name'] == package['name'] and p['version'] == version for p in locked):
        parser.error('Cargo.lock does not contain the release package version')
    if args.check:
        print(f'Validated {args.tag}')
        return
    if not args.target:
        parser.error('--target is required when packaging')
    executable = 'tileboard.exe' if 'windows' in args.target else 'tileboard'
    binary = ROOT / 'target' / args.target / 'release' / executable
    if not binary.is_file():
        parser.error(f'Build the native release binary first: {binary}')
    output = subprocess.check_output([str(binary), '--version'], text=True).strip()
    if output != f'tileboard {version}':
        parser.error(f'Binary version mismatch: {output!r}')
    for example in ['dashboard.toml', 'all-tiles.toml']:
        subprocess.run([str(binary), '--config', str(ROOT / 'examples' / example), '--check'], check=True)
    name = f'tileboard-{args.tag}-{args.target}'
    destination = ROOT / 'dist'
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='tileboard-package-') as directory:
        stage = pathlib.Path(directory) / name
        stage.mkdir()
        shutil.copy2(binary, stage / executable)
        if executable == 'tileboard':
            (stage / executable).chmod(0o755)
        for filename in ['README.md', 'CHANGELOG.md']:
            shutil.copy2(ROOT / filename, stage / filename)
        shutil.copytree(ROOT / 'docs', stage / 'docs')
        (stage / 'examples').mkdir()
        for filename in ['dashboard.toml', 'all-tiles.toml', 'usage.json']:
            shutil.copy2(ROOT / 'examples' / filename, stage / 'examples' / filename)
        archive = shutil.make_archive(str(destination / name), 'zip' if 'windows' in args.target else 'gztar', directory, name)
        print(f'Packaged {archive}')


if __name__ == '__main__':
    main()
