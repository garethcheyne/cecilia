#!/bin/sh
# Install Cecilia on Raspberry Pi OS (Lite or desktop), or any Debian-based
# Linux on a Pi or PC.
#
#   curl -fsSL https://raw.githubusercontent.com/garethcheyne/cecilia/main/packaging/install.sh | sudo sh
#
# It downloads the latest release for this computer, installs it as a
# service that starts at boot, names the machine "cecilia" on the network
# (unless CECILIA_KEEP_HOSTNAME=1), and prints the address to open.
set -eu

REPO="garethcheyne/cecilia"

say() { printf '\033[1m%s\033[0m\n' "$*"; }
fail() { printf 'Cecilia install: %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || fail "please run with sudo"
command -v apt-get >/dev/null || fail "this installer needs a Debian-based system such as Raspberry Pi OS"

case "$(dpkg --print-architecture)" in
    arm64) ARCH=arm64 ;;
    armhf) ARCH=armhf ;;
    amd64) ARCH=amd64 ;;
    *) fail "unsupported architecture $(dpkg --print-architecture)" ;;
esac

say "Finding the latest Cecilia for $ARCH…"
apt-get update -qq
apt-get install -y -qq curl ca-certificates avahi-daemon >/dev/null
URL=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep -o "https://[^\"]*_${ARCH}\.deb" | head -n 1)
[ -n "$URL" ] || fail "no release for $ARCH was found at https://github.com/$REPO/releases"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
say "Downloading $(basename "$URL")…"
curl -fsSL -o "$TMP/cecilia.deb" "$URL"

say "Installing…"
apt-get install -y -qq "$TMP/cecilia.deb" >/dev/null
# Keep the package, so Cecilia can go back to it after a later update.
VERSION=$(dpkg-deb -f "$TMP/cecilia.deb" Version | sed 's/-[0-9]*$//')
mkdir -p /var/lib/cecilia-updater && chmod 700 /var/lib/cecilia-updater
cp "$TMP/cecilia.deb" "/var/lib/cecilia-updater/cecilia_${VERSION}.deb"

if [ "${CECILIA_KEEP_HOSTNAME:-0}" != "1" ] && [ "$(hostname)" != "cecilia" ]; then
    say "Naming this computer \"cecilia\" on the network…"
    if command -v raspi-config >/dev/null; then
        raspi-config nonint do_hostname cecilia
    else
        hostnamectl set-hostname cecilia
    fi
    systemctl restart avahi-daemon || true
fi

say "Done. Cecilia is running."
echo
echo "  Open http://cecilia.local:7070 on a phone or computer on the same network."
echo "  (Or http://$(hostname -I | awk '{print $1}'):7070)"
echo
echo "  Next: set a password in Settings, then choose your MIDI interface in MIDI Setup."
echo "  Cecilia keeps itself up to date: see Settings, Updates."
