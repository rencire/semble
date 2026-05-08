# Semble

`semble` is a repo-aware host management CLI.

It expects the target repo to define a root-level `semble.toml` that specifies:
- repo paths such as `hosts/`, `ssh_host_keys/`, and `.sops.yaml`
- the host template location
- command-time orchestration settings such as builder policies

Image-specific prepare settings live in the image definition itself under
`prepare.partitionLabel`, not in `semble.toml`.

Typical commands:

```bash
# scaffold a new host directory and SSH host keys
semble host create my-host
# remove a host scaffold and related generated material
semble host delete my-host --yes
# build and switch a host configuration, prompting before activation
semble host switch my-host --target-host my-host-deploy --ask
# build and switch using a named strict builder policy from semble.toml
semble host switch my-host --target-host my-host-deploy --builder-policy buildbox
# install or reinstall NixOS on a remote target host
semble host provision my-host --target-host my-host-deploy
# provision a MicroVM guest image and bring it online through its parent host
semble host provision my-vm --disk-encryption-keys ./secrets/my-vm-root.key
```

Command behavior summary:
- `semble host build <host> [extra args...]`
  forwards to the equivalent of `tianyi os build . -H <host> [extra args...]`
- `semble host switch <host> [extra args...]`
  forwards to the equivalent of `tianyi os switch . -H <host> [extra args...]`
- `semble host provision <host> [extra args...]`
  forwards to the equivalent of `tianyi provision . -H <host> [extra args...]`
  - common option: `--builder-policy <name>`
  - physical-host passthrough: any trailing args after `--`, forwarded to
    `nixos-anywhere` as-is, including `--disk-encryption-keys` for full-disk
    encryption secrets
  - MicroVM-only options: `--disk-encryption-keys`, `--host-keys-dir`,
    `--system-store-path`, `--no-encrypt`, and `--force-reformat`
  - `--disk-encryption-keys` is for Semble's MicroVM guest provisioning path, not for
    `nixos-anywhere`
- `semble host keys ssh add|delete <host>` manages repository SSH host keys
- `semble host keys initrd-ssh add|delete <host>` manages initrd SSH host keys
- `semble host keys luks add|delete <host>` manages encrypted-root unlock keys
- `semble host provision <host> [extra args...]`
  resolves the guest's `microvm.volumes` configuration, creates or formats the
  root image on the parent host, copies the built system closure to the parent,
  installs into the mounted root, optionally copies SSH host keys into
  `/etc/ssh/`, validates the installed guest system, and restarts the MicroVM
  on the parent host
  - MicroVM-only options: `--disk-encryption-keys`, `--host-keys-dir`,
    `--system-store-path`, `--no-encrypt`, and `--force-reformat`
  - `--disk-encryption-keys` is required for encrypted provisioning and is staged into the
    MicroVM guest workflow by Semble
  - encrypted provisioning uses the `cryptroot` mapper name by default

Remote target note:
- `host switch` does not currently infer a remote deploy alias on its own
- for remote NixOS deployment, pass `--target-host` explicitly
- when `--target-host` is present, Semble now injects
  `--elevation-strategy passwordless` unless you already set an explicit
  elevation strategy
- `my-host-deploy` in the examples is an SSH host alias
- a normal SSH target such as `deploy@my-host.example.com` or `deploy@192.168.0.40`
  also works
- `--builder-policy <name>` selects a strict single-machine build policy from
  `semble.toml` for that invocation
- machine-level SSH alias installation belongs to the repo's Nix configuration,
  not to a Semble CLI setup command
- example:

```bash
semble host switch my-host --target-host my-host-deploy --ask
```
