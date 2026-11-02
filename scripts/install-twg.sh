#!/bin/sh
set -eu
# Official stable release, pinned and checked independently of the mutable manifest.
version=1.4.2
case "$(uname -m)" in
  aarch64|arm64) arch=arm64; checksum=c5bd21ea94eaa95446897b1eee072b38af60da2c88056120ae2294aade79f2c0 ;;
  x86_64|amd64) arch=x64; checksum=086662c6aa0d9e3d86c9f2f000115b8cc248d63152ab1f93e222e95515d05200 ;;
  *) echo 'Unsupported TWG architecture' >&2; exit 1 ;;
esac
artifact="/tmp/twg-linux-${arch}-v${version}"
trap 'rm -f "$artifact"' EXIT
curl -fsSL --retry 2 "https://teamwork-graph.atlassian.com/cli/twg-linux-${arch}-v${version}" -o "$artifact"
printf '%s  %s\n' "$checksum" "$artifact" | sha256sum -c -
install -m 0755 "$artifact" /usr/local/bin/twg
