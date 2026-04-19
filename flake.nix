{
  description = "Runtime pywal bridge for application theming";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        rec {
          default =
            let
              unwrapped = pkgs.rustPlatform.buildRustPackage {
                pname = "walbridge";
                version = "0.1.0";
                src = pkgs.lib.cleanSource ./.;
                cargoLock.lockFile = ./Cargo.lock;
                meta.mainProgram = "walbridge";
              };
            in
            pkgs.symlinkJoin {
              name = "walbridge-0.1.0";
              paths = [ unwrapped pkgs.adw-gtk3 ];
              nativeBuildInputs = [ pkgs.makeWrapper ];
              postBuild = ''
                gtk_base_theme=${pkgs.adw-gtk3}/share/themes/adw-gtk3-dark
                wrapProgram $out/bin/walbridge \
                  --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.glib pkgs.bat ]} \
                  --prefix XDG_DATA_DIRS : ${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name} \
                  --set WALBRIDGE_GTK_BASE_THEME_NAME adw-gtk3-dark \
                  --set WALBRIDGE_GTK3_BASE_CSS "$gtk_base_theme/gtk-3.0/gtk.css" \
                  --set WALBRIDGE_GTK4_BASE_CSS "$gtk_base_theme/gtk-4.0/gtk.css"
              '';
              meta.mainProgram = "walbridge";
            };

          walbridge = default;
        }
      );
    };
}
