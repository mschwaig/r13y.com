#!/usr/bin/env nix-shell
#!nix-shell -i bash ./default.nix -I nixpkgs=channel:nixos-unstable-small

set -eux

#export LANG=en_US.UTF-8
#export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

function nixpkgs_rev() (
    curl https://channels.nix.gsc.io/nixos-unstable-small/latest | cut -d' ' -f1
)

function main() {
    export ATTR="$1"
    export REPORT_NAME="$2"
    export REV=$(nixpkgs_rev)
    export HASH=$(nix flake prefetch nixpkgs/${REV})

    export RUST_BACKTRACE=1

    (
        unset RUST_LOG
        cargo build
    )

    cargo run -- \
        --flake nixpkgs/$REV\#$ATTR \
        --max-cores 48 \
        --max-cores-per-job 4 \
        check

    cargo run -- \
        --flake nixpkgs/$REV\#$ATTR \
        report

    mv ./report "./$REPORT_NAME"
}

main "$1" "$2"
