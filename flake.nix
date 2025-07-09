{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      overlays = [ (import rust-overlay) ];
      forAllSystems =
        function:
        nixpkgs.lib.genAttrs systems (
          system:
          let
            pkgs = import nixpkgs { inherit system overlays; };
          in
          function pkgs
        );
    in
    {
      devShells = forAllSystems (pkgs: {
        default =
          with pkgs;
          mkShell.override { stdenv = pkgs.clang12Stdenv; } rec {
            buildInputs = [
              (rust-bin.selectLatestNightlyWith (
                toolchain:
                toolchain.default.override {
                  extensions = [
                    "rust-src"
                    "rustfmt"
                  ];
                }
              ))
              rust-analyzer-unwrapped
              nixd
              pkg-config
              lua5_4
              libxkbcommon
              vulkan-loader
              vulkan-headers
              vulkan-validation-layers
              wgsl-analyzer
              wayland
              alsa-lib
            ];

            LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          };
      });

      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/package.nix { };
      });

      homeManagerModules = {
        default = import ./nix/home-manager.nix;
        stylix = import ./nix/stylix.nix;
      };
    };
}
