# Proposal: Declare provision flags in the host Nix definition

Status: `draft`

## Summary

Move the long tail of `nixos-anywhere` passthrough flags from the CLI
into the host's NixOS config, so `semble host provision my-host` works
without a multi-line command.

## Problem

The most common provision command requires remembering 7+ flags:

```bash
semble host provision my-host \
  --builder-policy buildbox \
  --target-host my-host-deploy \
  --host-keys-dir ./ssh_host_keys/my-host \
  --disk-encryption-keys /tmp/luks-root.key ./secrets/disk_keys/my-host/luks-root.key \
  --generate-hardware-config nixos-facter ./hosts/my-host/facter.json \
  --disko-mode disko \
  --phases disko,install,reboot
```

This is unwieldy and error-prone. Most of these values are per-host
constants that change infrequently — they belong in config, not on the
command line.

## Proposed Change

Declare provision flags in the host's Nix definition so they are
discoverable and repeatable. Semble would read them via `nix eval`,
the same way it already reads `prepare.partitionLabel` for
`image prepare`.

**Where the config lives is undecided.** Two candidates:

- **The host's NixOS module** (`configuration.nix`) — e.g.
  `semble.provision.target-host`. Feels natural since the host
  *is* this config, and the mechanism already exists
  (`prepare.partitionLabel` precedent).
- **The host's `default.nix`** — alongside the existing `_semble`
  metadata fields like `type`, `system`, `provisionTarget`. Keeps
  Semble metadata in one place, separate from the NixOS module tree.

The shape of the config would look something like this (regardless
of where it lives):

```nix
{
  target-host = "my-host-deploy";
  builder-policy = "buildbox";
  disko-mode = "disko";
  phases = [ "disko" "install" "reboot" ];
  build-on = "remote";              # optional, defaults to local
  disk-encryption-keys = {
    remote = "/tmp/luks-root.key";
    local = "./secrets/disk_keys/my-host/luks-root.key";
  };
  host-keys-dir = "./ssh_host_keys/my-host";
  generate-hardware-config = {
    backend = "nixos-facter";
    path = "./hosts/my-host/facter.json";
  };
}
```

The CLI would still accept the same flags, but they'd override the
config values when both are provided.

## CLI / Config Precedence

1. CLI flags win if present.
2. Config values fill in the rest.
3. Any missing required flag still produces a clear error.

## Open Questions

- Where should the config live — in the NixOS module or in
  `default.nix`?
- Should `_semble` host metadata (`type`, `system`, `provisionTarget`)
  be consolidated into the same block as part of this work, or kept
  separate?
- Should any provision fields remain CLI-only (e.g. `--phases` for
  ad-hoc debugging)?
