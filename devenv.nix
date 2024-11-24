{
  pkgs,
  lib,
  ...
}: let
  libs = with pkgs; [
    vulkan-headers
    vulkan-loader
    vulkan-tools
    vulkan-tools-lunarg
    vulkan-extension-layer
    vulkan-validation-layers

    libGL
    xorg.libX11
    xorg.libXi
    libxkbcommon
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXinerama
  ];
in {
  packages = libs;

  env.LD_LIBRARY_PATH = lib.makeLibraryPath libs;
}
