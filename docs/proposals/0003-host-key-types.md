# Proposal: Typed host key lifecycle commands

Status: `accepted`

## Summary

Replace the generic `host keys` shortcut with typed key-management commands so
Semble can manage multiple host key categories explicitly.

The initial key types would be:

- `ssh` for standard host SSH keys
- `initrd-ssh` for initrd SSH host keys used during provisioning
- `luks` for encrypted-root unlock keys used during provisioning

## Problem

Semble currently treats host keys as one broad category, but the repository now
needs to manage multiple distinct kinds of per-host key material.

That creates ambiguity:

- standard SSH host keys are repository-managed and participate in SOPS
- initrd SSH host keys are separate provisioning material
- LUKS root keys are raw unlock secrets and do not participate in SOPS

Using one untyped command makes the lifecycle and destination directories too
implicit.

## Proposed Change

Introduce typed key commands under `host keys`.

Preferred shape:

- `semble host keys ssh add <host>`
- `semble host keys ssh delete <host>`
- `semble host keys initrd-ssh add <host>`
- `semble host keys initrd-ssh delete <host>`
- `semble host keys luks add <host>`
- `semble host keys luks delete <host>`

The command should keep the current add/delete lifecycle flags where they still
make sense:

- `--force` for add
- `--yes` for delete
- `--skip-reencrypt` only for SSH key types that update SOPS
- `--sops-key-file` only for SSH key types that update SOPS

## Storage Layout

The destination directories should remain repo-configured via `semble.toml`.

Expected per-host layout:

- `ssh_host_keys/<host>/ssh_host_ed25519_key*`
- `initrd_ssh_host_keys/<host>/ssh_host_ed25519_key*`
- `luks_root_keys/<host>/root.key`

## Behavior

- `ssh` keys behave like the current `host keys` command.
- `initrd-ssh` keys use the same SSH key generation logic, but are stored in a
  separate directory for provisioning use.
- `luks` keys are raw 64-byte binary files generated from `/dev/urandom`.
- None of the new provisioning key types should participate in SOPS.

## Why This Is Better

- Key type is explicit in the command.
- Storage intent is clearer from the directory layout.
- Provisioning secrets are separated from ordinary SSH host identity keys.
- The CLI stays extensible if more key categories appear later.

## Migration Plan

1. Add typed `host keys` subcommands.
2. Refactor the existing SSH host-key command into the explicit `ssh` type.
3. Add `initrd-ssh` and `luks` generation paths.
4. Update docs and tests to use the typed forms.
5. Remove the old untyped shortcut.

## Open Questions

- Should `ssh` remain an alias for the old behavior during migration, or should
  users be forced to spell out the type immediately?
- Should the typed commands share one reusable add/delete implementation or be
  split by key family from the start?
