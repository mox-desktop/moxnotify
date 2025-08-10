{ ... }:
{
  projectRootFile = "flake.nix";

  settings.on-unmatched = "info";

  programs = {
    taplo.enable = true;
    rustfmt.enable = true;
    nixfmt = {
      enable = true;
      strict = true;
    };

    # https://github.com/google/keep-sorted
    keep-sorted = {
      enable = true;
      priority = 100; # run after other formatters
    };
  };
}
