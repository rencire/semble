# Proposal: Migrate encrypted MicroVM root provisioning into Semble

Status: `draft`

## Summary

Move the behavior currently implemented by the existing MicroVM provisioning
script in the sibling `rensemble` repo into Semble as a first-class MicroVM
provisioning command.

The goal is to stop carrying a separate shell script workflow in the personal
`rensemble` repo and make the provisioning path discoverable, testable, and
maintained alongside the rest of Semble's MicroVM support.

## Problem

The provisioning flow lives outside Semble today, which creates a split-brain
maintenance path:

- the Semble CLI already has MicroVM provisioning behavior
- the shell script in `rensemble` is a separate source of truth
- changes to provisioning semantics can drift between repositories
- the workflow is harder to discover from Semble's own documentation

## Current Script Workflow

The existing shell script currently does all of the following:

1. Validates the required arguments and optional flags.
2. Resolves the guest system to provision, either by running `semble host build`
   or by using a supplied `--system-store-path`.
3. Reads the guest's single supported `microvm.volumes` entry from the flake
   config and uses it as the source of truth for image path and size.
4. Refuses to proceed if the volume configuration is invalid, if there is more
   than one volume entry, or if encrypted provisioning is requested with a
   non-null `mountPoint`.
5. Skips provisioning entirely when `autoCreate=true`, since the microVM
   service will create the volume at boot.
6. Creates or truncates the backing image on the parent host when
   `autoCreate=false`, requiring `--force-reformat` if the image already exists.
7. For encrypted provisioning, copies the root unlock key to the parent host,
   runs `cryptsetup luksFormat`, opens the mapper, and creates ext4 on the
   mapper device.
8. For plain provisioning, creates ext4 directly on the image file.
9. Mounts the root filesystem on the parent host.
10. Copies the built system closure to the parent host and installs it into the
    mounted root with `nixos-install`.
11. Optionally copies SSH host keys into `/etc/ssh/` inside the guest root.
12. Verifies that the installed system profile exists, that `/etc/NIXOS` is
    present, and that SSH host keys were installed when requested.
13. Cleans up the remote mount, mapper, and temporary uploaded files on exit
    or failure.

## Proposed Change

Add a Semble `microvm provision` command so it can absorb the logic from the
external shell script and replace the existing `microvm provision-identity`
command.

The command should own the provisioning workflow end to end, including the
steps needed to prepare the MicroVM's encrypted root provisioning path.
It should also bring the guest online by handling the MicroVM host-side
activation step after the image is installed.

The current Semble `microvm provision-identity` command can inform the new
implementation, but the user-facing command should be broader than identity
setup if the script covers additional provisioning duties such as creating the
root image and copying the SSH files needed for identity.

For a higher-level comparison of the shared provisioning shape across
MicroVMs and physical hosts, see [docs/provisioning-flows.md](../provisioning-flows.md).

## Command Name

Preferred command shape:

- `semble microvm provision <guest> --parent <host>`

Why this shape:

- It keeps the command discoverable under the MicroVM namespace.
- It matches the actual task, which is guest provisioning on a parent host.
- It avoids turning `host provision` into a mode-switched command with a
  MicroVM-specific flag.
- It leaves room for a broader MicroVM command family later if Semble grows
  more guest-oriented workflows.
- It reflects that this workflow differs from physical-host provisioning:
  first the guest image is provisioned on the parent host, then the host-side
  Semble flow activates the MicroVM guest as part of the same command.

Rejected shape:

- `semble host provision --microvm`

This reads like host deployment with an optional backend switch, which does not
match the workflow as closely and would blur host provisioning with guest root
image preparation.

## Goals

- Keep the provisioning workflow in one place.
- Make MicroVM provisioning part of Semble's supported CLI surface.
- Reduce duplication between Semble and the sibling `rensemble` repo.
- Retire the narrower `microvm provision-identity` command once the new flow
  covers its behavior.
- Make the migration easier to document and test.

## Non-Goals

- Redesigning the MicroVM provisioning model from scratch.
- Changing unrelated host, image, or profile semantics.
- Making Semble responsible for generic remote automation beyond this workflow.

## Migration Plan

1. Migrate or port the existing `rensemble` test coverage into Semble tests or
   command-level fixtures.
2. Implement the Semble command so it reproduces that behavior.
3. Update Semble docs to point users at the new command and remove references
   to `microvm provision-identity`.
4. Remove or deprecate the old shell script in `rensemble` once the Semble
   version is stable.
5. Remove `microvm provision-identity` once the broader command covers its
   behavior.

## Open Questions

- Are there script behaviors that should stay in `rensemble` because they are
  repo-specific rather than Semble-specific?
- What compatibility story is needed for existing users of the shell script?

## Notes

This proposal is intentionally broader than an ADR. It is about moving a
workflow into Semble, not about changing Semble's long-lived architecture
model.
