# syntax=docker/dockerfile:1
# Image of the swiftlint mixin: the static Linux build of SwiftLint as /usr/local/bin/swiftlint, and its licenses.
# Version and digests: https://github.com/realm/SwiftLint/releases/tag/0.65.1 (2026-08-21), the sha256 of each
# swiftlint_linux_<arch>.zip as the release page lists it.
FROM docker.io/sbx/shell-image:latest AS build
USER root
ARG TARGETARCH
RUN <<'SH'
set -eu
V="0.65.1"
case "$TARGETARCH" in
  amd64) SUM="caeed6f4a679c35539ffaf124f6c4ab4a8416917f7d8796279dc52b74026059d" ;;
  arm64) SUM="9ffa52f478e6d8eb485d37d14715ffac90abc81c58f3370d598bf75be05605f8" ;;
  *)
    echo "unsupported arch: $TARGETARCH" >&2
    exit 1
    ;;
esac
WORK="$(mktemp -d)"
ZIP="$WORK/swiftlint.zip"
curl --proto '=https' --tlsv1.2 -fsSL -o "$ZIP" \
  "https://github.com/realm/SwiftLint/releases/download/${V}/swiftlint_linux_${TARGETARCH}.zip"
echo "${SUM}  $ZIP" | sha256sum -c -
unzip -q "$ZIP" swiftlint-static LICENSE LICENSE.mimalloc -d "$WORK"
mkdir -p /out/usr/local/bin /out/usr/local/share/doc/swiftlint
# The static build only: the dynamic one needs libxml2 and Foundation libraries the image lacks.
install -m 0755 "$WORK/swiftlint-static" /out/usr/local/bin/swiftlint
install -m 0644 "$WORK/LICENSE" "$WORK/LICENSE.mimalloc" /out/usr/local/share/doc/swiftlint/
rm -rf "$WORK"
test "$(/out/usr/local/bin/swiftlint version)" = "$V"
SH

FROM scratch
COPY --from=build /out/ /

# Marker: tells a process INSIDE the sandbox that this kit is on it (sbx exposes the kit list to
# the host only). See kits/statusline/statusline.dockerfile for why, and who reads it.
ENV SBX_KIT_SWIFTLINT=1
