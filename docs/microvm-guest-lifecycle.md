# MicroVM Guest Lifecycle

This note captures the lower-level steps involved in creating and provisioning a
MicroVM guest.

It complements [docs/provisioning-flows.md](./provisioning-flows.md), which
describes the shared high-level provisioning shape.

## Guest Creation Checklist

1. Define the parent host side.
   - Add the MicroVM runtime module to the parent host configuration.
   - Ensure the parent host knows how to attach and run the guest.
2. Define the guest in NixOS.
   - Add the guest's `nixosConfiguration`.
   - Define the guest's `microvm.volumes` entry.
   - Decide whether the root is encrypted or plain.
3. Define the boot-time root mapping.
   - If the guest uses encrypted storage, the initrd and filesystem config must
     agree on the mapper device name.
   - Semble currently uses `cryptroot` for the install-time mapping convention.
4. Prepare unlock material.
   - Generate the root unlock key used by the parent-host provisioning step.
   - If the guest uses remote unlock or SSH in the initrd, generate the
     corresponding initrd SSH host key material as well.
5. Prepare the host-side guest identity.
   - Generate or reuse SSH host keys for the guest runtime, if the guest is
     expected to expose SSH after boot.
   - Store those keys in the repo-managed guest key directory that Semble can
     copy into the image during provisioning.
6. Provision the guest image on the parent host.
   - Create or format the image.
   - Copy the system closure to the parent host.
   - Run `nixos-install` into the mounted image.
   - Install SSH host keys into the guest root when requested.
   - Validate the installed root.
7. Activate the guest on the parent host.
   - Deploy the parent host so the guest config is applied and the MicroVM
     services restart as part of that host update.

## Notes

- The exact key split depends on how the guest is configured.
- The install-time mapper name and the boot-time unlock name are separate
  concerns.
- This document describes the lifecycle steps; it does not prescribe the exact
  guest module structure.

## Relationship To Semble Commands

Today, Semble exposes the provisioning workflow through `semble host provision`.
That command currently covers step 6 only: it provisions the guest image on the
parent host and validates the installed root.

If the host-type dispatch proposal lands, the same guest lifecycle will be
reachable through the `host` command family, with the host type selecting the
appropriate strategy.
