#!/usr/bin/env bash
# Dependency and notice audit (M5-02). Fails when:
#   - cargo-deny finds an advisory, a yanked crate, a license outside
#     deny.toml's AGPL-compatible list, a banned crate or an unknown source;
#   - THIRD-PARTY-NOTICES no longer matches Cargo.lock and the crate sources
#     (regenerate with `python3 scripts/third_party_notices.py` and review);
#   - with --fuzz, a seeded property/fuzz suite fails (src/fuzz/).
# Needs network for the RustSec advisory database (git, github.com) and, the
# first time, for the pinned cargo-deny release. Never pushes or publishes.
#
# Usage: scripts/audit.sh [--tools-only] [--fuzz [CASES]]
#   SELA_TOOLS_DIR     where the pinned cargo-deny is kept (default ~/.cache/sela-tools)
#   SELA_ADVISORY_DB   advisory database folder (default: cargo-deny's own,
#                      $CARGO_HOME/advisory-dbs)
set -euo pipefail

DENY_VERSION=0.20.2
# SHA-256 of cargo-deny-$DENY_VERSION-x86_64-unknown-linux-musl.tar.gz from the
# upstream GitHub release, checked before the binary is used.
DENY_SHA256_LINUX=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
tools_only=
fuzz_cases=
while [[ $# -gt 0 ]]; do
    case $1 in
        --tools-only) tools_only=1 ;;
        --fuzz)
            fuzz_cases=2048
            if [[ ${2:-} =~ ^[0-9]+$ ]]; then
                fuzz_cases=$2
                shift
            fi
            ;;
        *) echo "usage: scripts/audit.sh [--tools-only] [--fuzz [CASES]]" >&2; exit 2 ;;
    esac
    shift
done

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
tools=${SELA_TOOLS_DIR:-$HOME/.cache/sela-tools}
deny=
find_deny() {
    local candidate
    for candidate in "$tools/cargo-deny-$DENY_VERSION/cargo-deny" "$(command -v cargo-deny || true)"; do
        if [[ -n $candidate && -x $candidate ]] &&
            [[ $("$candidate" --version 2>/dev/null) == "cargo-deny $DENY_VERSION" ]]; then
            deny=$candidate
            return 0
        fi
    done
    return 1
}
install_deny() {
    local name="cargo-deny-$DENY_VERSION-x86_64-unknown-linux-musl"
    local url="https://github.com/EmbarkStudios/cargo-deny/releases/download/$DENY_VERSION/$name.tar.gz"
    echo "audit: fetching cargo-deny $DENY_VERSION"
    curl -sSfL -o "$scratch/deny.tar.gz" "$url"
    echo "$DENY_SHA256_LINUX  $scratch/deny.tar.gz" | sha256sum -c --quiet -
    tar -xzf "$scratch/deny.tar.gz" -C "$scratch"
    mkdir -p "$tools/cargo-deny-$DENY_VERSION"
    install -m 755 "$scratch/$name/cargo-deny" "$tools/cargo-deny-$DENY_VERSION/cargo-deny"
}
if ! find_deny; then
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) install_deny ;;
        *)
            cargo install cargo-deny --locked --version "$DENY_VERSION" \
                --root "$tools/cargo-deny-$DENY_VERSION-build"
            mkdir -p "$tools/cargo-deny-$DENY_VERSION"
            cp "$tools/cargo-deny-$DENY_VERSION-build/bin/cargo-deny" "$tools/cargo-deny-$DENY_VERSION/"
            ;;
    esac
    find_deny || { echo "audit: cargo-deny $DENY_VERSION is not usable" >&2; exit 1; }
fi
echo "audit: $("$deny" --version) at $deny"
if [[ -n $tools_only ]]; then
    exit 0
fi

config=deny.toml
if [[ -n ${SELA_ADVISORY_DB:-} ]]; then
    # Same policy, advisory database kept elsewhere (for example a tmpfs).
    config="$scratch/deny.toml"
    sed "s|^\[advisories\]$|[advisories]\ndb-path = \"$SELA_ADVISORY_DB\"|" deny.toml >"$config"
fi

status=0
# The crate sources must exist locally for the notices; a no-op when they do.
cargo fetch --locked >/dev/null
if ! "$deny" --locked --config "$config" check advisories bans licenses sources; then
    echo "audit: cargo-deny check FAILED" >&2
    status=1
fi
# deny.toml allows cargo-deny's "wildcards" because it counts git
# dependencies as wildcards; this is the stricter rule it stands in for.
if ! cargo metadata --locked --offline --no-deps --format-version 1 | python3 -I -c '
import json, sys
bad = []
for package in json.load(sys.stdin)["packages"]:
    for dep in package["dependencies"]:
        name, req = dep["name"], dep["req"]
        source = dep.get("source") or ""
        if source.startswith("git+"):
            if "rev=" not in source:
                bad.append(name + ": git dependency without rev")
        elif dep.get("path") is None and not req.startswith("="):
            bad.append(name + " " + req + ": not an exact = pin")
for line in bad:
    print("audit: unpinned dependency " + line, file=sys.stderr)
sys.exit(1 if bad else 0)
'; then
    echo "audit: dependency pin check FAILED" >&2
    status=1
fi
if ! python3 -I scripts/test_third_party_notices.py 2>/dev/null; then
    echo "audit: notices generator unit checks FAILED" >&2
    status=1
fi
if ! python3 scripts/third_party_notices.py --check; then
    echo "audit: THIRD-PARTY-NOTICES check FAILED" >&2
    status=1
fi
if [[ -n $fuzz_cases ]]; then
    echo "audit: fuzz suites with $fuzz_cases cases per property"
    if ! PROPTEST_CASES=$fuzz_cases cargo test --locked -j 3 --lib fuzz::; then
        echo "audit: fuzz suites FAILED" >&2
        status=1
    fi
fi
if [[ $status == 0 ]]; then
    echo "audit: PASS"
fi
exit $status
