{
  stdenv,
  pnpm,
  nodejs,
  geist-font,
}:
stdenv.mkDerivation (finalAttrs: {
  pname = "moxnotify-webui";
  version = "0.1.0";

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
    inherit (finalAttrs) pname version src;
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
})
