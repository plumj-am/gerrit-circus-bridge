#!/usr/bin/env nu
try {
   nix flake check --all-systems --builders ""
} catch {
   print --stderr "Checks failed"
   exit 1
}

let version = open Cargo.toml | get package.version

print $"Current version: ($version)"
let new_version = input "Enter new version: "

if not ($new_version =~ '^[0-9]+\.[0-9]+\.[0-9]+$') {
    print --stderr "Error: Version must follow semantic versioning"
    exit 1
}

jj new

open Cargo.toml | upsert package.version $new_version | save -f Cargo.toml
cargo check --quiet

jj commit -m $"release: `v($new_version)`"
jj git export
git tag -f $"v($new_version)" --annotate --message $"v($new_version)"

cargo publish

print "Finished! Adjust the change and push it to the git host"
