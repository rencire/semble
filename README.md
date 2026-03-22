# Semble

`semble` is a repo-aware host management CLI.

It expects the target repo to define a root-level `semble.toml` that specifies:
- repo paths such as `hosts/`, `ssh_host_keys/`, `.sops.yaml`, and the SSH config module
- the host template location
- SSH alias conventions such as DNS suffix, users, and identity files

Typical commands:

```bash
semble host create thor
semble host delete thor --yes
semble host switch thor --ask
semble host provision thor --target-host thor-deploy
```
