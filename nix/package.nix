{
  rustPlatform,
  lib,
  pkg-config,
  lua5_4,
  libxkbcommon,
  wayland,
  vulkan-loader,
  pipewire,
  llvmPackages,
  libGL,
  egl-wayland,
}:
let
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "moxnotify";
  inherit (cargoToml.workspace.package) version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "pipewire-0.8.0" = "sha256-Ox3LaoFtl4KN+zfJgUEwNdwRCo3eyOSLQr+C18+H/sM=";
      "libspa-0.8.0" = "sha256-Ox3LaoFtl4KN+zfJgUEwNdwRCo3eyOSLQr+C18+H/sM=";
    };
  };

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "daemon"
        "ctl"
        "contrib"
        "pl.mox.notify.service.in"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    rustPlatform.bindgenHook
    llvmPackages.libclang
  ];

  buildInputs = [
    lua5_4
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
    install -Dm755 target/release/daemon $out/bin/moxnotifyd
    install -Dm755 target/release/ctl $out/bin/moxnotify
    ln -s moxnotify $out/bin/moxnotifyctl
  '';

  postFixup = ''
    mkdir -p $out/share/systemd/user
    substitute $src/contrib/systemd/moxnotify.service.in $out/share/systemd/user/moxnotify.service --replace-fail '@bindir@' "$out/bin"
    chmod 0644 $out/share/systemd/user/moxnotify.service

    mkdir -p $out/lib/systemd
    ln -s $out/share/systemd/user $out/lib/systemd/user

    mkdir -p $out/share/dbus-1/services
    substitute $src/pl.mox.notify.service.in $out/share/dbus-1/services/pl.mox.notify.service --replace-fail '@bindir@' "$out/bin"
    chmod 0644 $out/share/dbus-1/services/pl.mox.notify.service

    patchelf \
      --add-needed "${libGL}/lib/libEGL.so.1" \
      --add-needed "${vulkan-loader}/lib/libvulkan.so.1" \
      $out/bin/moxnotifyd
  '';

  dontPatchELF = false;

  meta = {
    description = "Mox desktop environment notification system";
    homepage = "https://github.com/mox-desktop/moxnotify";
    license = lib.licenses.mit;
    maintainers = builtins.attrValues { inherit (lib.maintainers) unixpariah; };
    platforms = lib.platforms.linux;
    mainProgram = "moxnotify";
  };
}
