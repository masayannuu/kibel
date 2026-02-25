#!/usr/bin/env bash
set -euo pipefail

TAG=""
REPO=""
LINUX_X86_64_SHA=""
LINUX_AARCH64_SHA=""
DARWIN_X86_64_SHA=""
DARWIN_AARCH64_SHA=""

usage() {
  cat <<'EOF' >&2
usage:
  render_homebrew_formula.sh \
    --tag vX.Y.Z \
    --repo owner/repo \
    --linux-x86_64-sha <sha256> \
    --linux-aarch64-sha <sha256> \
    --darwin-x86_64-sha <sha256> \
    --darwin-aarch64-sha <sha256>
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      TAG="${2:-}"
      shift 2
      ;;
    --repo)
      REPO="${2:-}"
      shift 2
      ;;
    --linux-x86_64-sha)
      LINUX_X86_64_SHA="${2:-}"
      shift 2
      ;;
    --linux-aarch64-sha)
      LINUX_AARCH64_SHA="${2:-}"
      shift 2
      ;;
    --darwin-x86_64-sha)
      DARWIN_X86_64_SHA="${2:-}"
      shift 2
      ;;
    --darwin-aarch64-sha)
      DARWIN_AARCH64_SHA="${2:-}"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ -z "${TAG}" || -z "${REPO}" || -z "${LINUX_X86_64_SHA}" || -z "${LINUX_AARCH64_SHA}" || -z "${DARWIN_X86_64_SHA}" || -z "${DARWIN_AARCH64_SHA}" ]]; then
  usage
  exit 2
fi

VERSION="${TAG#v}"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"

cat <<EOF
class Kibel < Formula
  desc "Production-focused Kibela CLI for Kibela GraphQL"
  homepage "https://github.com/${REPO}"
  version "${VERSION}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "${BASE_URL}/kibel-${TAG}-darwin-aarch64.tar.gz"
      sha256 "${DARWIN_AARCH64_SHA}"
    else
      url "${BASE_URL}/kibel-${TAG}-darwin-x86_64.tar.gz"
      sha256 "${DARWIN_X86_64_SHA}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "${BASE_URL}/kibel-${TAG}-linux-aarch64.tar.gz"
      sha256 "${LINUX_AARCH64_SHA}"
    else
      url "${BASE_URL}/kibel-${TAG}-linux-x86_64.tar.gz"
      sha256 "${LINUX_X86_64_SHA}"
    end
  end

  def install
    bin.install "kibel"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/kibel --version")
  end
end
EOF
