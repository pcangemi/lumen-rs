#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=2.5.1
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) platform=arm64.macos; checksum=98e0aca8e7ab69e9f4f0191582a500fd8b0d9085d68662f18ab6469e934a6efd ;;
  Darwin-x86_64) platform=x86_64.macos; checksum=a4341639005a33ce0d944bd7c771ce34de1465be7eb3189b7c3b5b0064429832 ;;
  Linux-x86_64) platform=x86_64.linux; checksum=743c3e2aff8c220d5d70fe6cb970fb3d36f2702d2693c61d1d148e404cf37cd6 ;;
  *) echo 'Install Open Image Denoise 2.x manually and set OIDN_LIBRARY to its shared library.'; exit 1 ;;
esac
archive=$(mktemp)
trap 'rm -f "$archive"' EXIT
curl -fL --retry 2 "https://github.com/RenderKit/oidn/releases/download/v${version}/oidn-${version}.${platform}.tar.gz" -o "$archive"
actual=$(shasum -a 256 "$archive" | cut -d ' ' -f 1)
if [[ "$actual" != "$checksum" ]]; then echo 'Download checksum mismatch'; exit 1; fi
mkdir -p .runtime/oidn
tar -xzf "$archive" --strip-components=1 -C .runtime/oidn
echo 'Open Image Denoise installed in .runtime/oidn. Its license remains in that directory.'
