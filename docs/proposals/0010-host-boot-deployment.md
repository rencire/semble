# Proposal: Add host boot deployment

Status: `draft`

## Table Of Contents

- [Summary](#summary)
- [Problem](#problem)
- [Proposed Change](#proposed-change)
- [Command Behavior](#command-behavior)
- [NixOS Semantics](#nixos-semantics)
- [CLI Shape](#cli-shape)
- [Documentation Updates](#documentation-updates)
- [Verification Plan](#verification-plan)
- [Out Of Scope](#out-of-scope)

## Summary

Add a middle-ground host operation that deploys a NixOS system as the next boot
generation without activating it live.

The proposed command is:

```bash
semble host boot <hostname> --target-host <ssh-destination>
```

This should behave like `nixos-rebuild boot`: build or copy the new system,
install it as the next boot generation, update the bootloader and current system
profile as appropriate, but do not run live activation or restart services.

## Problem

`semble host switch` is too aggressive for some host updates. On NixOS machines
with kernel-coupled userspace, such as hosts with NVIDIA driver or kernel
changes, live activation can put the running userspace ahead of the old running
kernel.

One failure mode is NVIDIA's driver/library mismatch:

```text
nvidia-smi: Failed to initialize NVML: Driver/library version mismatch
```

The safe operational flow is to install the new generation for next boot, reboot,
then verify the machine after the new kernel and userspace are running together.

Semble currently exposes:

- `semble host build`: build the host closure without deploying it
- `semble host switch`: deploy and activate the host immediately

There is no command for deploy-for-next-boot without live activation.

## Proposed Change

Add `semble host boot` for physical NixOS hosts.

The command should use the same target selection, builder policy, operator SSH
artifact regeneration, and argument-forwarding conventions as `semble host
switch`, but should ask the underlying NixOS deployment tool to install the
generation for boot rather than switch to it live.

This keeps the immediate fix focused on behavior. The command name follows the
existing `nixos-rebuild boot` vocabulary for now. A separate proposal should
evaluate whether Semble's long-term host lifecycle verbs should remain
`build`/`boot`/`switch` or use different product vocabulary.

## Command Behavior

`semble host boot` should:

- build the selected host configuration
- copy required build outputs to the target host when needed
- register the result as the target's next boot generation
- update bootloader state when the underlying NixOS flow normally does so
- avoid running activation scripts against the live system
- avoid restarting services as part of the deployment

After the command succeeds, the operator is responsible for rebooting and
verifying the host.

## NixOS Semantics

For NixOS hosts, Semble should map `host boot` to the boot operation of the
underlying deploy path.

If the path delegates to `nh`, Semble should use the closest equivalent of:

```bash
nh os boot . -H <hostname> ...
```

If a fallback path uses `nixos-rebuild`, Semble should use the closest equivalent
of:

```bash
nixos-rebuild boot --flake .#<hostname> --target-host <ssh-destination> ...
```

The implementation should verify the exact delegated command supported by the
current Semble host strategy before coding this behavior.

## CLI Shape

Example:

```bash
semble host boot my-host --target-host deploy@host.example.net --builder-policy preferred-linux-builder
```

Optional parameters should initially mirror `host switch` where the semantics are
identical:

| Flag | Purpose |
|------|---------|
| `--target-host <ssh-destination>` | SSH destination for the target machine. |
| `--builder-policy <policy>` | Apply a named builder policy defined in `semble.toml`. |

Extra arguments should be forwarded to the underlying deploy invocation using
the same conventions as `host switch`.

## Documentation Updates

Update host operation docs to describe three distinct lifecycle operations:

- `host build`: compile and verify the closure without deploying
- `host boot`: deploy as next boot generation without live activation
- `host switch`: deploy and activate immediately

The docs should call out kernel-coupled updates, especially NVIDIA/kernel
updates, as the primary reason to prefer `host boot` over `host switch`.

## Verification Plan

Implementation should include tests that cover:

- CLI parsing for `semble host boot`
- delegated command construction uses the boot action rather than switch
- `--target-host` and extra args are preserved
- `--builder-policy` behavior matches `host switch`
- host SSH artifact regeneration runs before `host boot` when it runs before
  `host build` and `host switch`

The full Semble test suite should pass after implementation:

```bash
cargo test
```

In this repository, run project commands through the Nix development shell.

## Out Of Scope

This proposal does not decide Semble's long-term command vocabulary. It also
does not add reboot orchestration, health checks, rollback automation, or
post-reboot verification. Those can be layered on after Semble has a safe
deploy-for-next-boot primitive.
