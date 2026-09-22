#!/bin/sh
# Install the latest Tileboard release. POSIX sh; no root privileges required.

set -eu

tileboard_error() {
    printf 'tileboard: %s\n' "$*" >&2
    exit 1
}

tileboard_quote() {
    printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

tileboard_download() {
    curl --fail --silent --show-error --location --retry 3 \
        --connect-timeout 15 --max-time 120 --proto '=https' --proto-redir '=https' \
        --tlsv1.2 "$@"
}

tileboard_profile() {
    mkdir -p "$(dirname "$1")"
    if [ -f "$1" ] && grep -Fqx -- "$tb_path_line" "$1"; then
        return
    fi
    printf '\n# Tileboard\n%s\n' "$tb_path_line" >> "$1" ||
        tileboard_error "Installed the binary, but could not update $1. Add $tb_bin_dir to PATH manually."
    printf 'Updated %s\n' "$1"
}

tileboard_main() {
    tb_version=''
    tb_bin_dir=${HOME:?HOME must be set}/.local/bin
    tb_profile=''
    tb_shell=${SHELL:-sh}
    tb_shell=${tb_shell##*/}
    tb_shell=${tb_shell:-sh}
    tb_modify_path=yes
    tb_tmp=''
    tb_stage=''
    trap '[ -z "$tb_stage" ] || rm -f "$tb_stage"; [ -z "$tb_tmp" ] || rm -rf "$tb_tmp"' 0
    trap 'exit 130' INT
    trap 'exit 143' TERM

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --version|--bin-dir|--profile|--shell)
                [ "$#" -ge 2 ] || tileboard_error "$1 requires a value"
                case "$1" in
                    --version) tb_version=$2 ;;
                    --bin-dir) tb_bin_dir=$2 ;;
                    --profile) tb_profile=$2 ;;
                    --shell) tb_shell=$2 ;;
                esac
                shift 2 ;;
            --no-modify-path) tb_modify_path=no; shift ;;
            -h|--help)
                cat <<'HELP'
Install Tileboard from GitHub Releases.

  curl -fsSL https://raw.githubusercontent.com/LimePencil/tileboard/main/install.sh | sh

Options (pass after: sh -s --):
  --version VERSION   Install a specific release, e.g. 0.1.0 (default: latest)
  --bin-dir PATH      Absolute installation directory (default: ~/.local/bin)
  --shell SHELL       Startup syntax: bash, zsh, fish, or sh (default: $SHELL)
  --profile PATH      Update this startup file instead of the shell's defaults
  --no-modify-path    Leave shell startup files and Windows User PATH unchanged
  --help             Show this help

Linux requires glibc 2.35+. macOS requires 11+. On Windows, run in Git Bash,
MSYS2, or Cygwin (x64). WSL installs the Linux binary. No sudo is used.
HELP
                return ;;
            *) tileboard_error "Unknown option: $1 (see --help)" ;;
        esac
    done

    # PATH entries and generated startup lines must be single, absolute paths.
    for tb_path in "$tb_bin_dir" "${tb_profile:-/unused}"; do
        case "$tb_path" in /*) ;; *) tileboard_error "Use an absolute path: $tb_path" ;; esac
        case "$tb_path" in *'
'*) tileboard_error 'Paths cannot contain newlines' ;; esac
        if printf '%s' "$tb_path" | LC_ALL=C grep -q '[[:cntrl:]:]'; then
            tileboard_error 'Paths cannot contain colons or control characters'
        fi
    done
    if [ "$tb_modify_path" = yes ]; then
        case "$tb_shell" in bash|zsh|fish|sh|dash|ksh) ;;
            *) tileboard_error "Unsupported shell $tb_shell; use --shell or --no-modify-path" ;;
        esac
    fi
    command -v curl >/dev/null 2>&1 || tileboard_error 'curl is required'
    tb_os=$(uname -s)
    tb_arch=$(uname -m)
    tb_windows=no
    tb_ext=tar.gz
    tb_binary=tileboard
    case "$tb_os" in
        Linux)
            tb_platform=unknown-linux-gnu
            tb_glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null || true)
            case "$tb_glibc" in 'glibc '*) ;; *) tileboard_error 'Linux binaries require glibc 2.35+; build from source on musl/other libc systems' ;; esac
            tb_glibc=${tb_glibc#glibc }
            awk -v version="$tb_glibc" 'BEGIN { split(version,v,"."); exit !(v[1]>2 || (v[1]==2 && v[2]>=35)) }' ||
                tileboard_error "glibc $tb_glibc is too old; need 2.35+ or a source build"
            ;;
        Darwin)
            tb_platform=apple-darwin
            if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then tb_arch=arm64; fi
            ;;
        MINGW*|MSYS*|CYGWIN*)
            tb_platform=pc-windows-msvc
            tb_windows=yes
            tb_ext=zip
            tb_binary=tileboard.exe
            if [ "$tb_modify_path" = yes ]; then
                command -v cygpath >/dev/null 2>&1 || tileboard_error 'cygpath is required to update Windows PATH'
                command -v powershell.exe >/dev/null 2>&1 || tileboard_error 'PowerShell is required to update Windows PATH; or use --no-modify-path'
            fi
            ;;
        *) tileboard_error "Unsupported operating system: $tb_os" ;;
    esac
    case "$tb_arch" in
        x86_64|amd64) tb_arch=x86_64 ;;
        aarch64|arm64) tb_arch=aarch64 ;;
        *) tileboard_error "Unsupported architecture: $tb_arch" ;;
    esac
    [ "$tb_windows:$tb_arch" != yes:aarch64 ] || tileboard_error 'Windows ARM64 releases are not available'
    tb_target=$tb_arch-$tb_platform
    if [ "$tb_ext" = zip ]; then tb_extract=unzip; else tb_extract=tar; fi
    command -v "$tb_extract" >/dev/null 2>&1 || tileboard_error "$tb_extract is required"
    if command -v sha256sum >/dev/null 2>&1; then tb_hash=sha256sum
    elif command -v shasum >/dev/null 2>&1; then tb_hash=shasum
    elif command -v openssl >/dev/null 2>&1; then tb_hash=openssl
    else tileboard_error 'A SHA-256 tool is required: sha256sum, shasum, or openssl'
    fi

    tb_repo=https://github.com/LimePencil/tileboard/releases
    if [ -z "$tb_version" ]; then
        tb_latest=$(tileboard_download --head --output /dev/null --write-out '%{url_effective}' "$tb_repo/latest") ||
            tileboard_error 'Could not resolve the latest release'
        case "$tb_latest" in "$tb_repo/tag/"*) tb_version=${tb_latest##*/} ;;
            *) tileboard_error "Unexpected latest release URL: $tb_latest" ;;
        esac
    fi
    tb_version=${tb_version#v}
    printf '%s\n' "$tb_version" | LC_ALL=C grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' ||
        tileboard_error 'Invalid release version; use a version such as 0.1.0'
    tb_root=tileboard-v$tb_version-$tb_target
    tb_archive=$tb_root.$tb_ext
    tb_tmp=$(mktemp -d "${TMPDIR:-/tmp}/tileboard-install.XXXXXXXX")
    printf 'Installing Tileboard v%s for %s…\n' "$tb_version" "$tb_target"
    tileboard_download --output "$tb_tmp/$tb_archive" "$tb_repo/download/v$tb_version/$tb_archive" ||
        tileboard_error 'Could not download the release archive'
    tileboard_download --output "$tb_tmp/SHA256SUMS" "$tb_repo/download/v$tb_version/SHA256SUMS" ||
        tileboard_error 'Could not download release checksums'
    tb_expected=$(awk -v file="$tb_archive" '$2 == file { print $1; count++ } END { if(count != 1) exit 1 }' "$tb_tmp/SHA256SUMS") ||
        tileboard_error 'Expected exactly one checksum for the release archive'
    [ "${#tb_expected}" -eq 64 ] || tileboard_error 'Invalid SHA-256 checksum'
    case "$tb_expected" in *[!0-9a-f]*) tileboard_error 'Invalid SHA-256 checksum' ;; esac
    case "$tb_hash" in
        sha256sum) tb_actual=$(sha256sum < "$tb_tmp/$tb_archive") ;;
        shasum) tb_actual=$(shasum -a 256 < "$tb_tmp/$tb_archive") ;;
        openssl) tb_actual=$(openssl dgst -sha256 < "$tb_tmp/$tb_archive"); tb_actual=${tb_actual##* } ;;
    esac
    tb_actual=${tb_actual%% *}
    [ "$tb_actual" = "$tb_expected" ] || tileboard_error 'Checksum mismatch; the existing installation was not changed'
    if [ "$tb_ext" = zip ]; then
        unzip -p "$tb_tmp/$tb_archive" "$tb_root/$tb_binary" > "$tb_tmp/$tb_binary"
    else
        tar -xzOf "$tb_tmp/$tb_archive" "$tb_root/$tb_binary" > "$tb_tmp/$tb_binary"
    fi
    chmod 755 "$tb_tmp/$tb_binary"
    tb_reported=$("$tb_tmp/$tb_binary" --version) ||
        tileboard_error 'The downloaded binary cannot run on this system; existing installation unchanged'
    [ "$tb_reported" = "tileboard $tb_version" ] || tileboard_error "Unexpected binary version: $tb_reported"
    mkdir -p "$tb_bin_dir"
    [ ! -d "$tb_bin_dir/$tb_binary" ] || tileboard_error "Installation destination is a directory: $tb_bin_dir/$tb_binary"
    tb_stage=$(mktemp "$tb_bin_dir/.tileboard.XXXXXXXX")
    cp "$tb_tmp/$tb_binary" "$tb_stage"
    chmod 755 "$tb_stage"
    mv -f "$tb_stage" "$tb_bin_dir/$tb_binary"
    tb_stage=''

    tb_quoted=$(tileboard_quote "$tb_bin_dir")
    # Comparing complete colon-delimited entries avoids both duplicate PATH entries and globs.
    tb_path_line="case \":\$PATH:\" in *:$tb_quoted:*) ;; *) export PATH=$tb_quoted:\"\$PATH\" ;; esac"
    if [ "$tb_shell" = fish ]; then
        tb_fish_quoted=$(printf '%s' "$tb_bin_dir" | sed "s/\\\\/\\\\\\\\/g; s/'/\\\\'/g")
        tb_path_line="contains -- '$tb_fish_quoted' \$PATH; or set -gx PATH '$tb_fish_quoted' \$PATH"
    fi
    if [ "$tb_modify_path" = yes ]; then
        if [ -n "$tb_profile" ]; then
            tileboard_profile "$tb_profile"
        else
            case "$tb_shell" in
                bash)
                    tileboard_profile "$HOME/.bashrc"
                    if [ -f "$HOME/.bash_profile" ]; then tileboard_profile "$HOME/.bash_profile"
                    elif [ -f "$HOME/.bash_login" ]; then tileboard_profile "$HOME/.bash_login"
                    else tileboard_profile "$HOME/.profile"
                    fi ;;
                zsh)
                    tileboard_profile "${ZDOTDIR:-$HOME}/.zshrc"
                    tileboard_profile "${ZDOTDIR:-$HOME}/.zprofile" ;;
                fish) tileboard_profile "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/tileboard.fish" ;;
                *) tileboard_profile "$HOME/.profile" ;;
            esac
        fi
        if [ "$tb_windows" = yes ]; then
            TILEBOARD_WINDOWS_INSTALL_DIR=$(cygpath -w "$tb_bin_dir")
            export TILEBOARD_WINDOWS_INSTALL_DIR
            # These variables belong to PowerShell, not the calling shell.
            # shellcheck disable=SC2016
            powershell.exe -NoProfile -NonInteractive -Command '
                $ErrorActionPreference = "Stop"
                $dir = $env:TILEBOARD_WINDOWS_INSTALL_DIR
                $old = [Environment]::GetEnvironmentVariable("Path", "User")
                $entries = @($old -split ";" | Where-Object { $_ })
                $expanded = @($entries | ForEach-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd("\") })
                if ($expanded -notcontains $dir.TrimEnd("\")) {
                    [Environment]::SetEnvironmentVariable("Path", ((@($dir) + $entries) -join ";"), "User")
                }
            ' || tileboard_error 'Installed the binary, but could not update Windows User PATH'
        fi
    fi
    printf '\nInstalled %s\n' "$tb_bin_dir/$tb_binary"
    if [ "$tb_modify_path" = yes ]; then
        printf 'Open a new terminal and run: tileboard\nOr update this terminal now:\n  %s\n' "$tb_path_line"
    else
        printf 'Add %s to PATH to run tileboard by name.\n' "$tb_bin_dir"
    fi
}

# Keep this invocation last so a truncated script cannot start installation.
tileboard_main "$@"
