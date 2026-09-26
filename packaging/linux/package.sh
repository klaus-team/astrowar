#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: package.sh <version> <binary> <license> <outdir>" >&2
  exit 2
fi

version="$1"
binary="$2"
license="$3"
outdir="$4"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version must be X.Y.Z, got: $version" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
desktop="$script_dir/astrowar.desktop"

for path in "$binary" "$license" "$desktop"; do
  if [[ ! -f "$path" ]]; then
    echo "missing file: $path" >&2
    exit 2
  fi
done

binary="$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")"
license="$(cd "$(dirname "$license")" && pwd)/$(basename "$license")"
mkdir -p "$outdir"
outdir="$(cd "$outdir" && pwd)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

deb_root="$work/deb"
mkdir -p \
  "$deb_root/usr/bin" \
  "$deb_root/usr/share/applications" \
  "$deb_root/usr/share/doc/astrowar" \
  "$work/src/debian"
install -m 755 "$binary" "$deb_root/usr/bin/astrowar"
install -m 644 "$desktop" "$deb_root/usr/share/applications/astrowar.desktop"
install -m 644 "$license" "$deb_root/usr/share/doc/astrowar/copyright"
mkdir -p "$work/src/debian/astrowar"
cp -a "$deb_root/usr" "$work/src/debian/astrowar/"
cat >"$work/src/debian/control" <<'EOF'
Source: astrowar

Package: astrowar
Architecture: amd64
EOF

depends="$(
  cd "$work/src"
  dpkg-shlibdeps -O debian/astrowar/usr/bin/astrowar \
    | sed -n 's/^shlibs:Depends=//p'
)"
if [[ -z "$depends" ]]; then
  echo "dpkg-shlibdeps produced no Depends" >&2
  exit 1
fi

mkdir -p "$deb_root/DEBIAN"
cat >"$deb_root/DEBIAN/control" <<EOF
Package: astrowar
Version: ${version}
Section: games
Priority: optional
Architecture: amd64
Maintainer: AstroWar Maintainers <noreply@users.noreply.github.com>
Homepage: https://github.com/pauloklaus/astrowar
Depends: ${depends}
Description: Multiplayer arcade shooter
 Native client. Rooms are joined through a central relay.
EOF

dpkg-deb --root-owner-group --build "$deb_root" \
  "$outdir/astrowar_${version}_amd64.deb"

rpm_top="$work/rpm"
mkdir -p "$rpm_top"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cp "$binary" "$rpm_top/SOURCES/astrowar"
cp "$desktop" "$rpm_top/SOURCES/astrowar.desktop"
cp "$license" "$rpm_top/SOURCES/LICENSE"
cat >"$rpm_top/SPECS/astrowar.spec" <<'EOF'
Name: astrowar
Version: @VERSION@
Release: 1
Summary: Multiplayer arcade shooter
License: MIT
URL: https://github.com/pauloklaus/astrowar
BuildArch: x86_64

%global __os_install_post %{nil}
%global debug_package %{nil}

%description
Native client. Rooms are joined through a central relay.

%install
install -D -m 755 %{_sourcedir}/astrowar %{buildroot}/usr/bin/astrowar
install -D -m 644 %{_sourcedir}/astrowar.desktop %{buildroot}/usr/share/applications/astrowar.desktop
install -D -m 644 %{_sourcedir}/LICENSE %{buildroot}/usr/share/licenses/astrowar/LICENSE

%files
/usr/bin/astrowar
/usr/share/applications/astrowar.desktop
/usr/share/licenses/astrowar/LICENSE
EOF
sed -i "s/@VERSION@/${version}/" "$rpm_top/SPECS/astrowar.spec"

rpmbuild \
  --define "_topdir ${rpm_top}" \
  --define "_build_id_links none" \
  --define "dist %{nil}" \
  -bb "$rpm_top/SPECS/astrowar.spec"
cp "$rpm_top/RPMS/x86_64/astrowar-${version}-1.x86_64.rpm" "$outdir/"

arch_root="$work/arch"
mkdir -p \
  "$arch_root/usr/bin" \
  "$arch_root/usr/share/applications" \
  "$arch_root/usr/share/licenses/astrowar"
install -m 755 "$binary" "$arch_root/usr/bin/astrowar"
install -m 644 "$desktop" "$arch_root/usr/share/applications/astrowar.desktop"
install -m 644 "$license" "$arch_root/usr/share/licenses/astrowar/LICENSE"
size="$(du -sb "$arch_root/usr" | cut -f1)"
cat >"$arch_root/.PKGINFO" <<EOF
pkgname = astrowar
pkgbase = astrowar
pkgver = ${version}-1
pkgdesc = Multiplayer arcade shooter
url = https://github.com/pauloklaus/astrowar
builddate = $(date +%s)
packager = AstroWar Maintainers <noreply@users.noreply.github.com>
size = ${size}
arch = x86_64
license = MIT
depend = gcc-libs
depend = glibc
depend = openssl
depend = alsa-lib
depend = systemd-libs
depend = wayland
depend = libxkbcommon
depend = libxkbcommon-x11
depend = libx11
depend = libxcb
depend = libxcursor
depend = libxi
depend = libxrandr
depend = libxinerama
depend = libxext
depend = vulkan-icd-loader
EOF

(
  cd "$arch_root"
  bsdtar --zstd -cf "$outdir/astrowar-${version}-1-x86_64.pkg.tar.zst" .PKGINFO usr
)
