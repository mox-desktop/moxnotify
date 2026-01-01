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
    };
  };
}
