# Semble

`semble` is a repo-aware host management CLI.

It expects the target repo to define a root-level `semble.toml` that specifies:
- repo paths such as `hosts/`, `ssh_host_keys/`, and `.sops.yaml`
- the host template location
- the SSH symlink path to refresh during `ssh setup`
- SSH alias conventions such as DNS suffix, users, and identity files

Image-specific prepare settings live in the image definition itself under
`prepare.partitionLabel`, not in `semble.toml`.

Typical commands:

```bash
# scaffold a new host directory and SSH host keys
semble host create atlas
# remove a host scaffold and related generated material
semble host delete atlas --yes
# regenerate the managed SSH alias include file
semble ssh setup
# build and switch a host configuration, prompting before activation
semble host switch atlas --target-host atlas-deploy --ask
# build and switch using a named strict builder policy from semble.toml
semble host switch atlas --target-host atlas-deploy --builder-policy l380y
# install or reinstall NixOS on a remote target host
semble host provision atlas --target-host atlas-deploy
```

Command behavior summary:
- `semble host build <host> [extra args...]`
  forwards to the equivalent of `tianyi os build . -H <host> [extra args...]`
- `semble host switch <host> [extra args...]`
  forwards to the equivalent of `tianyi os switch . -H <host> [extra args...]`
- `semble host provision <host> [extra args...]`
  forwards to the equivalent of `tianyi provision . -H <host> [extra args...]`

Remote target note:
- `host switch` does not currently infer a remote deploy alias on its own
- for remote NixOS deployment, pass `--target-host` explicitly
- `atlas-deploy` in the examples is an SSH host alias
- a normal SSH target such as `deploy@atlas.example.com` or `deploy@192.168.0.40`
  also works
- `--builder-policy <name>` selects a strict single-machine build policy from
  `semble.toml` for that invocation
- example:

```bash
semble host switch atlas --target-host atlas-deploy --ask
```
