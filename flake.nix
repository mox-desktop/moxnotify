{
  inputs.tooling.url = "github:mox-desktop/tooling";

  outputs =
    { self, tooling, ... }:
    tooling.lib.mkMoxFlake {
      devShells = tooling.lib.forAllSystems (pkgs: {
        default = pkgs.mkShell.override { stdenv = pkgs.clang12Stdenv; } (
          pkgs.lib.fix (finalAttrs: {
            buildInputs = builtins.attrValues {
              inherit (pkgs)
                rustToolchain
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
                libGL
                egl-wayland
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
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.moxnotify;
      });

      homeManagerModules = {
        moxnotify = import ./nix/home-manager.nix;
        default = self.homeManagerModules.moxnotify;
      };
    };
}
