#!/usr/bin/env bash

main() {
    export LAPIN_CODEGEN_DIR="$(dirname "${0}" | xargs realpath)/src/generated"

    cargo build --features=codegen-internal
    rustfmt "${LAPIN_CODEGEN_DIR}"/*.rs --edition=2024
}

main "${@}"
