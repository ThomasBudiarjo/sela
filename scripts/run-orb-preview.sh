#!/usr/bin/env bash
# Foreground Desktop launcher for the locally installed orb technical preview.
# Desktop supplies DISPLAY/Wayland/audio; never override those here.
set -euo pipefail
repo="$(dirname "$(dirname "$(readlink -f "$0")")")"
# This orb's GPUI Vulkan path rendered black; use the verified Mesa GL path.
export VK_DRIVER_FILES=/dev/null
exec "$repo/.amp/install/bin/sela" "$@"
