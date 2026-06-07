{
  description = "Gerrit Circus CI Bridge";

  nixConfig = {
    builders-use-substitutes = true;
    flake-registry = "";
    show-trace = true;

    experimental-features = [
      "flakes"
      "nix-command"
    ];

    extra-substituters = [
      "https://cache.garnix.io/"
      "https://nix-community.cachix.org/"
    ];

    extra-trusted-public-keys = [
      "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    fenix.url = "github:nix-community/fenix";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      crane,
      ...
    }:
    let
      lib = nixpkgs.lib;

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      eachSystem =
        f:
        lib.genAttrs systems (
          system:
          let
            pkgs = nixpkgs.legacyPackages.${system};
            craneLib = (crane.mkLib pkgs).overrideToolchain fenix.packages.${system}.complete.toolchain;
          in
          f {
            inherit
              self
              lib
              system
              pkgs
              craneLib
              ;
          }
        );

      pname = "gerrit-circus-bridge";
    in
    {
      packages = eachSystem (
        { craneLib, ... }:
        let
          src = craneLib.cleanCargoSource ./.;
          gerrit-circus-bridge = craneLib.buildPackage {
            inherit pname src;
            strictDeps = true;
            meta.mainProgram = pname;
          };
        in
        {
          inherit gerrit-circus-bridge;
          default = gerrit-circus-bridge;
        }
      );

      checks = eachSystem ({ craneLib, lib, ... }: import ./nix/checks.nix { inherit craneLib lib; });

      devShells = eachSystem (
        { craneLib, system, ... }:
        {
          default = craneLib.devShell {
            checks = self.checks.${system};
          };
        }
      );

      nixosModules.default = {
        imports = [ ./nix/module.nix ];
        nixpkgs.overlays = [ self.overlays.default ];
      };

      overlays.default = final: _: {
        gerrit-circus-bridge = self.packages.${final.system}.default;
      };
    };
}
