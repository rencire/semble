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
# provision a MicroVM guest SSH host identity through its parent host
semble microvm provision-identity my-vm --parent my-host
```

Command behavior summary:
- `semble host build <host> [extra args...]`
  forwards to the equivalent of `tianyi os build . -H <host> [extra args...]`
- `semble host switch <host> [extra args...]`
  forwards to the equivalent of `tianyi os switch . -H <host> [extra args...]`
- `semble host provision <host> [extra args...]`
  forwards to the equivalent of `tianyi provision . -H <host> [extra args...]`
- `semble microvm provision-identity <host> --parent <parent>`
  stages `ssh_host_keys/<host>/ssh_host_ed25519_key*` under
  `/run/microvm-provisioning/<host>` on the parent host, restarts the MicroVM,
  waits for the guest to acknowledge persistence, and removes the staged key
  material from the parent
  - by default the parent SSH target is `<parent>-admin`
  - pass `--target-host` to override the SSH target
  - pass `--replace` for migrations where an existing persistent guest key
    should be replaced by the repo-managed key

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
