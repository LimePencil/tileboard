#!/usr/bin/env python3
"""Exercise the real terminal UI with tmux; uses only temporary configuration files.
Run: cargo build --locked && python3 scripts/ui_smoke.py
Requires Python 3.11+ and tmux on Linux/macOS.
"""
import os
import pathlib
import shlex
import subprocess
import tempfile
import time
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/debug/tileboard'
SOCKET = f'tileboard-smoke-{os.getpid()}'
BASE = ['tmux', '-L', SOCKET]


def tmux(*args):
    return subprocess.check_output(BASE + list(args), text=True, timeout=10)


def capture():
    return tmux('capture-pane', '-p', '-t', 'dashboard')


def wait_for(text):
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        screen = capture()
        if text in screen:
            return screen
        time.sleep(.05)
    raise AssertionError(f'Missing {text!r}:\n{screen}')


def keys(*values):
    tmux('send-keys', '-t', 'dashboard', *values)
    time.sleep(.18)


def literal(value):
    tmux('send-keys', '-t', 'dashboard', '-l', value)
    time.sleep(.18)


def paste(value):
    literal('\x1b[200~' + value + '\x1b[201~')


def main():
    with tempfile.TemporaryDirectory(prefix='tileboard-ui-') as directory:
        config = pathlib.Path(directory) / 'dashboard.toml'
        command = shlex.join([str(BINARY), '--config', str(config)])
        tmux('new-session', '-d', '-s', 'dashboard', '-x', '120', '-y', '32', command)
        try:
            tmux('set-option', '-g', 'status', 'off')
            screen = wait_for('logical CPUs')
            for title in ['Memory', 'Network', 'System', 'Storage', 'Local time']:
                assert title in screen
            assert 'available' in screen and 'receive' in screen and 'Up ' in screen
            for width, height, profile in [(80, 24, 'compact'), (58, 28, 'small'), (38, 40, 'tall'), (120, 12, 'short'), (26, 10, 'minimal')]:
                tmux('resize-window', '-t', 'dashboard', '-x', str(width), '-y', str(height))
                time.sleep(.2)
                screen = capture()
                assert 'Enlarge tile' not in screen, screen
                keys('e')
                wait_for(profile + ' profile')
                assert 'Esc cancel' in capture(), capture()
                keys('Escape')
            tmux('resize-window', '-t', 'dashboard', '-x', '120', '-y', '32')
            wait_for('wide')
            keys('e', 'Right')
            wait_for('Blocked')
            keys('Enter')
            wait_for('Blocked')
            keys('Escape', 'h', 'Enter')
            # SGR mouse: move the shrunken CPU tile one cell to the right.
            literal('\x1b[<0;4;5M')
            literal('\x1b[<32;24;5M')
            literal('\x1b[<0;24;5m')
            wait_for('Placement applied')
            keys('s')
            wait_for('Saved')
            saved = tomllib.loads(config.read_text())
            placement = saved['profiles'][0]['tiles'][0]['placement']
            assert placement['column'] == 1 and placement['column_span'] == 1, placement
            # Unicode paste, cursor editing, and saved settings.
            keys('e', 't', 'C-u')
            paste('My 한글 CPU')
            keys('Home')
            paste('New ')
            keys('Enter', 's')
            wait_for('Saved')
            saved = tomllib.loads(config.read_text())
            assert saved['profiles'][0]['tiles'][0]['title'] == 'New My 한글 CPU'
            # Delete then cancel restores the complete session without touching disk.
            before = config.read_bytes()
            keys('e', 'd', 'Escape')
            wait_for('Edit session cancelled')
            assert 'New My 한글 CPU' in capture()
            assert config.read_bytes() == before
            # Replace the network tile through the registry picker and test missing data.
            keys('e', 'Tab', 'Tab', 'Tab', 'Tab', 'd', 'a', 'Down', 'Down', 'Down', 'Enter')
            wait_for('Tile added')
            keys('t', 'Tab', 'Tab', 'C-u')
            paste('nonexistent-interface')
            keys('Enter')
            wait_for('Interface unavailable')
            keys('t', 'Tab', 'Tab', 'C-u', 'Enter', 's')
            wait_for('Saved')
            assert len(tomllib.loads(config.read_text())['profiles'][0]['tiles']) == 6
            # Reload errors preserve the live layout and the invalid external file.
            config.write_text('broken = [')
            keys('r')
            wait_for('Reload failed')
            assert config.read_text() == 'broken = ['
            assert 'New My 한글 CPU' in capture()
            keys('q')
            assert subprocess.run(BASE + ['has-session', '-t', 'dashboard'], capture_output=True).returncode != 0
            print('PASS: six live tiles, five responsive shapes, readable controls, collision preview, mouse move, save, Unicode input, add/settings, cancel, reload failure, clean exit')
        finally:
            subprocess.run(BASE + ['kill-server'], capture_output=True)


if __name__ == '__main__':
    main()
