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
  protobuf,
  nodejs,
  pnpm,
  geist-font,
  stdenv,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);

  webui = stdenv.mkDerivation {
    pname = "moxnotify-webui";
    version = cargoToml.workspace.package.version;

    src = ../webui;

    nativeBuildInputs = [
      nodejs
      pnpm.configHook
    ];

    buildInputs = [
      geist-font
    ];

    env = {
      CI = "true";
      NIX_BUILD = "1";
    };

    pnpmDeps = pnpm.fetchDeps {
      pname = "moxnotify-webui";
      version = cargoToml.workspace.package.version;
      src = ../webui;
      fetcherVersion = 2;
      hash = "sha256-1HZHSZr85L0EGrf1rTt3nGEOirRLX+sCj51/7hZN3wg=";
    };

    buildPhase = ''
      runHook preBuild

      mkdir -p app/fonts

      cp ${geist-font}/share/fonts/opentype/Geist-Regular.otf app/fonts/Geist-Regular.otf
      cp ${geist-font}/share/fonts/opentype/Geist-Bold.otf app/fonts/Geist-Bold.otf
      cp ${geist-font}/share/fonts/opentype/GeistMono-Regular.otf app/fonts/GeistMono-Regular.otf

      pnpm run build

      runHook postBuild
    '';

    installPhase = ''
      mkdir -p $out
      cp -r .next $out/.next
      cp -r public $out/public
      cp package.json $out/
      cp -r node_modules $out/node_modules
    '';
  };
in
rustPlatform.buildRustPackage {
  pname = "moxnotify";
  inherit (cargoToml.workspace.package) version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "moxui-0.1.0" = "sha256-i2jwHkdoJW99mcb+DqCqhMfigNz4Bc1QmGcThWl2bRM=";
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
        "collector"
        "control_plane"
        "indexer"
        "proto"
        "scheduler"
        "searcher"
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
    install -Dm755 target/release/client $out/bin/moxnotify-client
    install -Dm755 target/release/collector $out/bin/moxnotify-collector
    install -Dm755 target/release/control_plane $out/bin/moxnotify-control-plane
    install -Dm755 target/release/indexer $out/bin/moxnotify-indexer
    install -Dm755 target/release/scheduler $out/bin/moxnotify-scheduler
    install -Dm755 target/release/searcher $out/bin/moxnotify-searcher
    install -Dm755 target/release/ctl $out/bin/moxnotifyctl

    mkdir -p $out/share/moxnotify/webui
    cp -r ${webui}/.next $out/share/moxnotify/webui/.next
    cp -r ${webui}/public $out/share/moxnotify/webui/public
    cp ${webui}/package.json $out/share/moxnotify/webui/
    cp -r ${webui}/node_modules $out/share/moxnotify/webui/node_modules
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
