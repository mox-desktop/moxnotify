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
      styles = [
        {
          selector = "*";
          style = {
            font = {
              family = fonts.sansSerif.name;
              size = fonts.sizes.popups;
              color = base05;
            };
            border.color = {
              urgency_low = base0B;
              urgency_normal = base0E;
              urgency_critical = base08;
            };
          };
        }
        {
          selector = [
            "next_counter"
            "prev_counter"
            "notification"
            "hints"
            "summary"
          ];
          style.background = {
            urgency_low = base00 + moxnotifyOpacity;
            urgency_normal = base01 + moxnotifyOpacity;
            urgency_critical = base01 + moxnotifyOpacity;
          };
        }

        {
          selector = [
            "notification"
            "summary"
          ];
          state = "hover";
          style.background = {
            urgency_low = base02 + moxnotifyOpacity;
            urgency_normal = base02 + moxnotifyOpacity;
            urgency_critical = base02 + moxnotifyOpacity;
          };
        }
        {
          selector = "action";
          state = "hover";
          style.background = {
            urgency_low = base0F;
            urgency_normal = base0F;
            urgency_critical = base08;
          };
        }
        {
          selector = "progress";
          style.background = {
            urgency_low = base0F;
            urgency_normal = base0F;
            urgency_critical = base08;
          };
        }
        {
          selector = "dismiss";
          style.font.color = "#00000000";
        }
        {
          selector = "dismiss";
          state = "container_hover";
          style.font.color = "#000000";
        }
      ];

      #css =
      #  # css
      #  ''
      #      * {
      #        font-family: ${fonts.sansSerif.name};
      #        font-size: ${toString fonts.sizes.popups}px;
      #        color: ${base05}
      #      }

      #      .notification.low * {
      #        border-color: ${base0B};
      #      }

      #      .notification.normal * {
      #        border-color: ${base0E};
      #      }

      #      .notification.critical * {
      #        border-color: ${base08};
      #      }

      #      .notification.critical,
      #      .notification.critical .next_counter,
      #      .notification.critical .prev_counter,
      #      .notification.critical .hints,
      #      .notification.critical .summary {
      #          background-color: ${base01 + moxnotifyOpacity};
      #      }

      #      .notification.normal,
      #      .notification.normal .next_counter,
      #      .notification.normal .prev_counter,
      #      .notification.normal .hints,
      #      .notification.normal .summary {
      #          background-color: ${base01 + moxnotifyOpacity};
      #      }

      #      .notification.low,
      #      .notification.low .next_counter,
      #      .notification.low .prev_counter,
      #      .notification.low .hints,
      #      .notification.low .summary {
      #          background-color: ${base00 + moxnotifyOpacity};
      #      }

      #    .notification:hover {
      #        background-color: ${base02 + moxnotifyOpacity};
      #    }

      #    .notification.critical .action:hover {
      #        background-color: ${base08};
      #    }

      #    .notification.normal .action:hover {
      #        background-color: ${base0F};
      #    }

      #    .notification.low .action:hover {
      #        background-color: ${base0F};
      #    }

      #    .notification.low .progress {
      #        background-color: ${base0F};
      #    }

      #    .notification.normal .progress {
      #        background-color: ${base0F};
      #    }

      #    .notification.critical .progress {
      #        background-color: ${base08};
      #    }

      #    .dismiss {
      #        color: #00000000;
      #    }

      #    .dismiss:hover {
      #        color: #000000;
      #    }
      #  '';
    };
  };
}
