{ inputs, ... }:
[
  inputs.llm-agents.overlays.shared-nixpkgs
  (final: prev: {
    entire = inputs.entire-cli-flake.packages.${final.system}.default;
  })
]
