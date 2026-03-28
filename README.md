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
semble host create thor
semble host delete thor --yes
semble ssh setup
semble host switch thor --ask
semble host provision thor --target-host thor-deploy
```
