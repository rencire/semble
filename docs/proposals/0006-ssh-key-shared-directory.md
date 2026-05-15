# Proposal: Share SSH key directory between normal and initrd keys

Status: `accepted`

## Summary

Normal and initrd SSH host keys now share a single directory
(`ssh_host_keys/<host>`) with distinct filenames instead of using
separate directories. The `initrd_ssh_host_keys_dir` config field
is removed.

## Problem

Proposal 0003 introduced separate directories for each key type:

- `ssh_host_keys/<host>/ssh_host_ed25519_key`
- `initrd_ssh_host_keys/<host>/ssh_host_ed25519_key`

This required a distinct `initrd_ssh_host_keys_dir` config field in
`semble.toml` and a parallel set of path methods in the repo plumbing.
Both key types serve the same machine and follow the same generation
logic — the only difference is which NixOS module consumes them.

## Motivation

The root-unlock workflow requires initrd SSH host keys to be available in
a persistent store on the target machine. The `ssh_host_keys` directory
is already copied to the host as part of the physical provisioning
workflow, so placing initrd keys there as well means they are available
at boot time without adding a separate copy step or a second persisted
path.

Reusing the same directory was the simplest way to ensure initrd keys
survive across reboots and are present when the early-boot SSH server
needs them for remote unlock.

## Solution

Place both key types in the same `ssh_host_keys/<host>` directory with
distinct filenames:

- `ssh_host_keys/<host>/ssh_host_ed25519_key` (normal SSH)
- `ssh_host_keys/<host>/initrd_ssh_host_ed25519_key` (initrd SSH)

### Changes

- `KeyKind` uses the same directory for both `Ssh` and `InitrdSsh` variants.
- `generate_ssh_keypair` creates the directory if missing, checks for
  existing target files, and removes only the matching file on `--force`.
- Delete removes only the files matching the key kind, then removes the
  parent directory only if empty — preventing one key type from wiping
  the other.
- `initrd_ssh_host_keys_dir` field removed from `PathsConfig` and all
  related `RepoPaths` methods deleted.
- Config entry removed from all fixture `semble.toml` strings.

## Benefits

1. **Fewer config fields** — no need to configure a separate path per
   key type.
2. **Simpler directory tree** — one directory per host regardless of
   how many key types exist.
3. **Safe coexistence** — targeted file matching means delete only
   touches its own key type. A directory is only removed when empty.
