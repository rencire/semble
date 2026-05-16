# Host Operations

Day-to-day commands for building and deploying host configurations after a host
has already been provisioned. For first-time installation, see
[provisioning-flows.md](./provisioning-flows.md).

## Prerequisites

Your deploy SSH key must be loaded in your SSH agent before running any command
that connects to a host as the deploy user:

```bash
ssh-add /path/to/your/deploy_key
```

If the key is not in the agent, SSH will accept the public key challenge but
fail to sign it, resulting in a `Permission denied (publickey)` error that can
be hard to diagnose.

## semble host ssh generate

Generates operator SSH artifacts from host manifest `operator` metadata:

```bash
semble host ssh generate
```

Outputs:

- `.semble/cache/ssh/semble-servers.conf`
- `.semble/cache/ssh/known_hosts`

Hosts with `operator.role = "server"` contribute aliases. Hosts with
`operator.role = "client"` automatically regenerate these artifacts before
`semble host build` and `semble host switch`.

## semble host build

Builds the host configuration closure without deploying it. Useful for
verifying a config compiles before deploying, or pre-building on a remote
builder before switching.

```bash
semble host build <hostname>
```

Example:

```bash
semble host build my-host
```

Extra args are forwarded to the underlying Nix build invocation.

### Optional parameters

| Flag | Purpose |
|------|---------|
| `--builder-policy <policy>` | Pin the build to a named remote builder defined in `semble.toml`. See [builder-policies.md](./builder-policies.md). |

## semble host switch

Builds and deploys the host configuration to the target, then activates it.
This is the standard command for updating a running host.

```bash
semble host switch <hostname> --target-host <ssh-destination>
```

Example:

```bash
semble host switch my-host --target-host deploy@my-host.example.net
```

Extra args are forwarded to the underlying deploy invocation.

### Optional parameters

| Flag | Purpose |
|------|---------|
| `--builder-policy <policy>` | Pin the build to a named remote builder defined in `semble.toml`. See [builder-policies.md](./builder-policies.md). |

## Build then switch

Pre-building before switching is useful when you want to verify the build
succeeds on a specific builder before touching the running host:

```bash
semble host build my-host --builder-policy buildbox
semble host switch my-host --target-host deploy@my-host.example.net --builder-policy buildbox
```
