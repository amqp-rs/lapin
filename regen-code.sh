#!/usr/bin/env bash
set -euo pipefail

main() {
    local root
    root="$(cd "$(dirname "${0}")" && pwd -P)"
    export LAPIN_CODEGEN_DIR="${root}/src/generated"

    cargo build --manifest-path "${root}/Cargo.toml" --features=codegen-internal
    rustfmt "${LAPIN_CODEGEN_DIR}"/*.rs --edition=2024
}

main "${@}"
