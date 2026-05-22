#!/usr/bin/env bash
# Stop hook: block newly modified files from defining canonical cross-crate
# types in the wrong crate. Full-workspace drift can be inspected with:
#   python3 harness/check_type_vocabulary.py --audit

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

python3 harness/check_type_vocabulary.py
