{
  rustPlatform,
  lib,
  pkg-config,
  lua5_4,
  libxkbcommon,
  wayland,
  vulkan-loader,
  alsa-lib,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../daemon/Cargo.toml);
in
rustPlatform.buildRustPackage rec {
  pname = "moxnotify";
  inherit (cargoToml.package) version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "moxui-0.1.0" = "sha256-v/4a0+ljKu8vag9suBxZIi12CKwT7xorYy/Am03xtY0=";
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

  nativeBuildInputs = [ pkg-config ];

  buildInputs = [
    lua5_4
    libxkbcommon
    wayland
    vulkan-loader
    alsa-lib
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

    patchelf --set-rpath "${lib.makeLibraryPath buildInputs}" $out/bin/moxnotifyd
  '';

  dontPatchELF = false;

  meta = with lib; {
    description = "Mox desktop environment notification system";
    homepage = "https://github.com/mox-desktop/moxnotify";
    license = licenses.mit;
    maintainers = [ maintainers.unixpariah ];
    platforms = platforms.linux;
    mainProgram = "moxnotify";
  };
}
