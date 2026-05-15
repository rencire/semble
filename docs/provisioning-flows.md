# Provisioning Flows

This note captures the shared shape of Semble-style guest provisioning and
remote host installation workflows.

The details differ, but the high-level pattern is similar:

1. Define the desired NixOS configuration up front.
2. Prepare storage or disk layout.
3. Transfer the build output or closure needed for installation.
4. Install the system into the prepared target.
5. Reboot or activate the newly provisioned system.

The ownership of those steps differs by workflow:

- `semble host provision` owns steps 2, 3, 4, and 5 for physical-host
  installation.
- `semble host provision` should own steps 2, 3, 4, and 5 for MicroVM guest
  provisioning, including the host-side activation/start step.

For physical hosts, `host provision` forwards trailing passthrough arguments
directly to `nixos-anywhere`. Use `--disk-encryption-keys` there for encrypted
disk setup; Semble does not interpret that flag itself. The MicroVM-only flags
on `host provision` are `--disk-encryption-keys`, `--host-keys-dir`,
`--system-store-path`, `--no-encrypt`, and `--force-reformat`.

`--disk-encryption-keys` is only for Semble's MicroVM provisioning path, where
it stages the guest root unlock key before the image is installed.

## Practical Examples

### Physical Host Provision

All flags after the hostname are forwarded directly to `nixos-anywhere`. No `--`
separator is needed.

**Basic provision:**

```bash
semble host provision my-host --target-host my-host-deploy
```

**With disk encryption and hardware config:**

```bash
semble host provision my-host \
  --target-host my-host-deploy \
  --disk-encryption-keys /tmp/luks-key /path/to/key \
  --generate-hardware-config nixos-facter ./hosts/my-host/facter.json
```

**Full example (all common flags):**

```bash
semble host provision my-host \
  --builder-policy buildbox \
  --target-host my-host-deploy \
  --host-keys-dir ./ssh_host_keys/my-host \
  --disk-encryption-keys /tmp/luks-key ./secrets/my-host/luks-root.key \
  --generate-hardware-config nixos-facter ./hosts/my-host/facter.json \
  --disko-mode disko \
  --phases disko,install,reboot \
  --build-on remote
```

### MicroVM Provision

All flags are Semble flags interpreted directly (no passthrough to external
tools).

**Basic encrypted provision:**

```bash
semble host provision my-vm --disk-encryption-keys ./secrets/my-vm-root.key
```

**With SSH host keys:**

```bash
semble host provision my-vm \
  --disk-encryption-keys ./secrets/my-vm-root.key \
  --host-keys-dir ./ssh_host_keys/my-vm
```

### Flag Reference

| Flag                         | Physical Host  | MicroVM                      | Source                  |
| ---------------------------- | -------------- | ---------------------------- | ----------------------- |
| `--builder-policy`           | ✅ Semble flag | ✅ Semble flag               | Semble                  |
| `--disk-encryption-keys`     | ✅ Forwarded   | ✅ MicroVM flag\*            | nixos-anywhere / Semble |
| `--host-keys-dir`            | ✅ Forwarded   | ✅ MicroVM flag              | Semble                  |
| `--no-encrypt`               | ❌ Error       | ✅ Optional                  | Semble                  |
| `--system-store-path`        | ❌ Error       | ✅ Optional                  | Semble                  |
| `--force-reformat`           | ❌ Error       | ✅ Optional                  | Semble                  |
| `--target-host`              | ✅ Forwarded   | ❌ Use `provisionTarget`\*\* | nixos-anywhere          |
| `--generate-hardware-config` | ✅ Forwarded   | ❌                           | nixos-anywhere          |
| `--disko-mode`               | ✅ Forwarded   | ❌                           | nixos-anywhere          |
| `--phases`                   | ✅ Forwarded   | ❌                           | nixos-anywhere          |
| `--build-on`                 | ✅ Forwarded   | ❌                           | nixos-anywhere          |

\* For MicroVM, `--disk-encryption-keys` takes one argument (the local key
path).  
\*\* MicroVM uses `provisionTarget` from host config in `semble.toml`

## MicroVM Guest Provisioning

In the MicroVM case, the target is a guest image managed on the parent host. For
the lower-level guest setup checklist, see
[docs/microvm-guest-lifecycle.md](./microvm-guest-lifecycle.md).

1. The guest configuration is defined first.
2. The root image is created or formatted on the parent host.
3. The Nix store closure is copied to the parent host.
4. `nixos-install` installs into the mounted image.
5. The guest is then started or switched as part of the provisioning command.

## Physical Host Installation

In the physical-host case, the target is a remote machine.

1. The host configuration is defined first.
2. `disko` prepares the physical disks on the target machine.
3. The Nix store closure is transferred to the remote installer environment.
4. `nixos-install` installs into the mounted target filesystem.
5. The machine is then rebooted into the new system.

## Why This Matters

The flows share a broad structure, but the provisioning target is different:

- MicroVM provisioning targets a guest image on a parent host.
- Physical-host provisioning targets a real remote machine.

That distinction is why Semble should keep separate command shapes for these
workflows even if the high-level steps look similar.

## Host Type Comparison

This table compares the current public command shape against the two host types
discussed in the lifecycle docs.

| Command          | Physical host                                                                        | MicroVM host                                                                                         |
| ---------------- | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `host create`    | Scaffold a normal host directory from the selected template.                         | Scaffold a MicroVM-backed host definition and note the parent-host requirement.                      |
| `host build`     | Build the host config for a real machine.                                            | Build the guest config that will become the MicroVM image.                                           |
| `host switch`    | Deploy the host config to the physical machine and activate it there.                | Deploy the parent host config so it wires up and starts the MicroVM guest.                           |
| `host provision` | Install the system onto the target machine, including disk prep and reboot/activate. | Provision the guest image on the parent host, then require a parent-host switch to make it runnable. |
| `host delete`    | Remove the host scaffold, keys, and related metadata.                                | Remove the MicroVM host scaffold, guest identity, and related metadata.                              |

## Under The Hood

Those command-owned steps are implemented differently:

- The MicroVM path uses Semble's own logic for guest-image provisioning and
  parent-host activation.
- The physical-host path can delegate the remote install mechanics to
  `nixos-anywhere` and `disko`, while Semble still owns the command-level
  orchestration.
