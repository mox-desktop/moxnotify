{
  inputs.tooling.url = "git+https://forgejo.r0chd.pl/mox-desktop/tooling.git";

  outputs =
    { self, tooling, ... }:
    tooling.lib.mkMoxFlake {
      devShells = tooling.lib.forAllSystems (pkgs: {
        default = pkgs.mkShell.override { stdenv = pkgs.clangStdenv; } (
          pkgs.lib.fix (finalAttrs: {
            buildInputs = builtins.attrValues {
              inherit (pkgs)
                rustToolchain
                rust-analyzer-unwrapped
                nixd
                pkg-config
                libxkbcommon
                vulkan-loader
                vulkan-headers
                vulkan-validation-layers
                wgsl-analyzer
                wayland
                pipewire
                libnotify
                libclang
                libGL
                egl-wayland
                protobuf
                pnpm
                nodejs_25
                tilt
                valkey
                protols
                ;
            };
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath finalAttrs.buildInputs;
            RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
          })
        );
      });

      packages = tooling.lib.forAllSystems (pkgs: {
        moxnotify = pkgs.callPackage ./nix/package.nix {
          rustPlatform = pkgs.makeRustPlatform {
            cargo = pkgs.rustToolchain;
            rustc = pkgs.rustToolchain;
          };
        };
        moxnotify-webui = pkgs.callPackage ./nix/webui.nix { };
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.moxnotify;
      });

      homeManagerModules = {
        moxnotify = import ./nix/home-manager.nix;
        default = self.homeManagerModules.moxnotify;
      };

      overlays.default =
        final: prev:
        let
          inherit (prev.stdenv.hostPlatform) system;
        in
        {
          inherit (self.packages.${system}) moxnotify;
          inherit (self.packages.${system}) webui;
        };
    };
}
