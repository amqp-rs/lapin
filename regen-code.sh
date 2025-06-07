#!/usr/bin/env bash

main() {
    export LAPIN_CODEGEN_DIR="$(dirname "${0}" | xargs realpath)/src/"
    export LAPIN_CODEGEN_FILE="generated"

    cargo build --features=codegen-internal
    rustfmt "${LAPIN_CODEGEN_DIR}/${LAPIN_CODEGEN_FILE}.rs" --edition=2024
}

main "${@}"
