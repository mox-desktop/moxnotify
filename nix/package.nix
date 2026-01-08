{
  rustPlatform,
  lib,
  pkg-config,
  libxkbcommon,
  wayland,
  vulkan-loader,
  pipewire,
  llvmPackages,
  libGL,
  egl-wayland,
  protobuf,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "moxnotify";
  inherit (cargoToml.workspace.package) version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "moxui-0.1.0" = "sha256-bmPGQV1ybazf0fmrrJLV3q54GIUH5d3Pt4MRQAKXjF0=";
      "tvix-eval-0.1.0" = "sha256-OVcABz/8InamZLdF3AvD1FW13aL3Nd3BZ2vkjfMEjkM=";
    };
  };

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      p: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString p);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "client"
        "ctl"
        "collector-dbus"
        "control_plane"
        "indexer"
        "janitor"
        "proto"
        "scheduler"
        "searcher"
        "config"
        "pl.mox.notify.service.in"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    rustPlatform.bindgenHook
    llvmPackages.libclang
    protobuf
  ];

  buildInputs = [
    libxkbcommon
    wayland
    pipewire
    egl-wayland
  ];

  doCheck = false;

  buildPhase = ''
    cargo build --release --workspace
  '';

  installPhase = ''
    install -Dm755 target/release/client $out/bin/moxnotify-client
    install -Dm755 target/release/collector-dbus $out/bin/moxnotify-collector
    install -Dm755 target/release/control_plane $out/bin/moxnotify-control-plane
    install -Dm755 target/release/indexer $out/bin/moxnotify-indexer
    install -Dm755 target/release/janitor $out/bin/moxnotify-janitor
    install -Dm755 target/release/scheduler $out/bin/moxnotify-scheduler
    install -Dm755 target/release/searcher $out/bin/moxnotify-searcher
    install -Dm755 target/release/ctl $out/bin/moxnotifyctl
  '';

  postFixup = ''
    patchelf \
      --add-needed "${libGL}/lib/libEGL.so.1" \
      --add-needed "${vulkan-loader}/lib/libvulkan.so.1" \
      $out/bin/moxnotify-client
  '';

  dontPatchELF = false;

  meta = {
    description = "Mox desktop environment notification system";
    homepage = "https://forgejo.r0chd.pl/mox-desktop/moxnotify";
    license = lib.licenses.gpl3;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
  };
}
