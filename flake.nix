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
          let
            inherit (pkgs) lib;
            buildInputs = [
              (pkgs.rust-bin.stable.latest.default.override {
                extensions = [
                  "rust-src"
                  "rustfmt"
                ];
              })
            ]
            ++ builtins.attrValues {
              inherit (pkgs)
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
                pipewire
                libnotify
                libclang
                ;
            };
          in
          pkgs.mkShell.override { stdenv = pkgs.clang12Stdenv; } {
            inherit buildInputs;
            LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
          };
      });

      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/package.nix {
          rustPlatform =
            let
              rust-bin = pkgs.rust-bin.stable.latest.default;
            in
            pkgs.makeRustPlatform {
              cargo = rust-bin;
              rustc = rust-bin;
            };
        };
      });

      homeManagerModules = {
        default = import ./nix/home-manager.nix;
        stylix = import ./nix/stylix.nix;
      };
    };
}
