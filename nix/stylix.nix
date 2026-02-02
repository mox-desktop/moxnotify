{ lib, config, ... }:
let
  moxnotifyOpacity = lib.toHexString (
    ((builtins.floor (config.stylix.opacity.popups * 100 + 0.5)) * 255) / 100
  );
  inherit (config.stylix) fonts;
  inherit (config.lib.stylix.colors.withHashtag)
    base05
    base0B
    base0E
    base08
    base00
    base01
    base02
    base0F
    ;
in
{
  options.stylix.targets.moxnotify.enable = config.lib.stylix.mkEnableTarget "moxnotify" true;

  config = lib.mkIf (config.stylix.enable && config.stylix.targets.moxnotify.enable) {
    services.moxnotify.client.settings = {
      css =
        # css
        ''
          .notification {
            font-family: ${fonts.sansSerif.name};
            font-size: ${toString fonts.sizes.popups}px;
            color: ${base05};
          }

          .notification.low {
            border-color: ${base0B};
            background-color: ${base00 + moxnotifyOpacity};
          }

          .notification.normal {
            border-color: ${base0E};
            background-color: ${base01 + moxnotifyOpacity};
          }

          .notification.critical {
            border-color: ${base08};
            background-color: ${base01 + moxnotifyOpacity};
          }

          .notification.low:hover {
            background-color: ${base02 + moxnotifyOpacity};
          }

          .notification.normal:hover {
            background-color: ${base02 + moxnotifyOpacity};
          }

          .notification.critical:hover {
            background-color: ${base02 + moxnotifyOpacity};
          }

          .notification.low .summary {
            background-color: ${base00 + moxnotifyOpacity};
          }

          .notification.normal .summary {
            background-color: ${base01 + moxnotifyOpacity};
          }

          .notification.critical .summary {
            background-color: ${base01 + moxnotifyOpacity};
          }

          .notification.low .hint {
            background-color: ${base00 + moxnotifyOpacity};
          }

          .notification.normal .hint {
            background-color: ${base01 + moxnotifyOpacity};
          }

          .notification.critical .hint {
            background-color: ${base01 + moxnotifyOpacity};
          }

          .counter {
            background-color: ${base01 + moxnotifyOpacity};
          }

          .notification.low .button.action:hover {
            background-color: ${base0F};
          }

          .notification.normal .button.action:hover {
            background-color: ${base0F};
          }

          .notification.critical .button.action:hover {
            background-color: ${base08};
          }

          .notification.low .progress {
            background-color: ${base0F};
          }

          .notification.normal .progress {
            background-color: ${base0F};
          }

          .notification.critical .progress {
            background-color: ${base08};
          }

          .button.dismiss {
            color: #00000000;
          }

          .button.dismiss:hover {
            color: #000000;
          }
        '';
    };
  };
}
