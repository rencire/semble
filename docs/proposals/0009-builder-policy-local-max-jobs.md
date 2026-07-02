# Proposal: Structure builder policy scheduling

Status: `draft`

## Table Of Contents

- [Summary](#summary)
- [Problem](#problem)
- [Proposed Change](#proposed-change)
- [Configuration Shape](#configuration-shape)
- [Field Semantics](#field-semantics)
- [CLI Behavior](#cli-behavior)
- [Provision Behavior](#provision-behavior)
- [Default Behavior](#default-behavior)
- [Verification Plan](#verification-plan)
- [Out Of Scope](#out-of-scope)

## Summary

Restructure `semble.toml` builder policies around same-machine build settings
and delegated builder machine specs.

`local.max_jobs` controls the local Nix daemon job count that Semble passes to
delegated Nix or `nh` commands. `builders[].max_jobs` remains the delegated
builder slot count serialized into each Nix builder machine spec.

## Problem

Semble currently treats a selected builder policy as strict builder-only
selection. When `--builder-policy` is used, Semble injects both a serialized
delegated builder and a local daemon job limit of zero:

```bash
--builders '<serialized delegated builder>' --max-jobs 0
```

The provision path applies the same local limit through `NIX_CONFIG`:

```bash
max-jobs = 0
```

This prevents same-machine builds for the command. That strict behavior is
useful when the operator wants to guarantee that the local Nix daemon does not
run builds, but it breaks uncached derivations that set `preferLocalBuild = true`.
For those derivations, Nix can try the local or semi-local build path first. If
Semble has forced `--max-jobs 0`, no local daemon slots are available, and the
build can fail with a misleading message about delegated machines or required
features.

## Proposed Change

Represent each builder policy as a named bundle of local Nix scheduling settings
and one or more delegated builder machine specs.

When a builder policy is selected, Semble should use that policy's
`local.max_jobs` value for the local daemon's `--max-jobs` setting. If the field
is absent, Semble should keep the current strict behavior and use `0`.

This keeps delegated builder capacity and local daemon capacity as separate
knobs without relying on ambiguous flat field names:

- `local.max_jobs`: same-machine Nix daemon job count for this Semble invocation
- `builders[].max_jobs`: delegated builder machine slot count

## Configuration Shape

Example builder policy:

```toml
[[builder_policies]]
name = "thor-aarch64-builder-preferred"

[builder_policies.local]
max_jobs = 1

[[builder_policies.builders]]
host = "thor-builder"
system = "aarch64-linux"
ssh_key = "/etc/nix/ssh/homelab_builder"
max_jobs = 6
speed_factor = 20
supported_features = [
  "benchmark",
  "big-parallel",
  "kvm",
  "nixos-test",
  "linux-native-host-builder",
]
```

## Field Semantics

`name` remains the CLI-facing policy identifier used by `--builder-policy`. The
name can include the intended strategy, such as `builder-only`,
`builder-preferred`, or `pool`, without requiring a separate `mode` field.

`local.max_jobs` is optional. When present, Semble uses it as the local daemon
job count for the delegated command. A value of `1` allows one same-machine
build slot, which is useful for uncached `preferLocalBuild` derivations while
still keeping most work on delegated builders.

If `local.max_jobs` is omitted, Semble should default it to `0` to preserve the
current strict behavior.

Each `builders[]` entry is a delegated Nix builder machine spec. Its `max_jobs`,
`speed_factor`, and `supported_features` fields are serialized into the Nix
builder machine entry passed through `--builders` or `NIX_CONFIG builders = ...`.

Using `builders[]` instead of a singular `builder` keeps the policy shape aligned
with Nix's `builders` setting and allows Nix to choose among several delegated
builders based on system, features, speed factor, and load.

## CLI Behavior

For host build and switch commands that delegate to `nh`, a selected builder
policy should continue to inject the selected delegated builders. The value
after `--max-jobs` should come from `local.max_jobs`, defaulting to `0` when
omitted.

For the example policy above, Semble would emit the delegated builder's
`max_jobs = 6` inside the builder spec and the local daemon's
`local.max_jobs = 1` as the `--max-jobs` value:

```bash
--builders 'ssh://thor-builder aarch64-linux /etc/nix/ssh/homelab_builder 6 20 benchmark,big-parallel,kvm,nixos-test,linux-native-host-builder' --max-jobs 1
```

If a policy defines multiple `builders[]` entries, Semble should serialize each
entry into the Nix builders setting using Nix's supported multi-builder syntax.

## Provision Behavior

For provision commands that invoke `nixos-anywhere`, Semble should merge both
the delegated builders and local max-jobs settings into `NIX_CONFIG`. This
matches the Nix CLI configuration mechanism used by the `nix build` commands
that `nixos-anywhere` runs internally:

```bash
NIX_CONFIG='builders = <serialized delegated builders>
max-jobs = 1'
```

If `NIX_CONFIG` already contains other settings, Semble should continue to
append the policy's builders and local max-jobs lines without dropping existing
lines.

## Default Behavior

The default for omitted `local.max_jobs` is `0`.

This preserves the current first-version strict behavior for existing consumer
repositories. Repositories opt in to local daemon build slots only by adding the
new local max-jobs field to a specific builder policy.

## Verification Plan

Implementation should include tests that cover:

- absent `local.max_jobs` keeps delegated `--max-jobs 0`
- `local.max_jobs = 1` emits delegated `--max-jobs 1`
- provision setup merges `builders = ...` and `max-jobs = 1` into `NIX_CONFIG`
- each `builders[].max_jobs` is still serialized into the builder machine spec
- multiple `builders[]` entries are serialized into one Nix builders setting

The full Semble test suite should pass after the change:

```bash
cargo test
```

In the Semble repository, run project commands through the Nix development shell
as described by the repo workflow.

## Out Of Scope

This proposal does not convert builder policies to Nix or add broader
Semble-managed builder routing modes such as ordered fallback chains, dynamic
builder discovery, cache warming, or command-specific routing.
