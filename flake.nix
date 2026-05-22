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
          workspaceSrc = pkgs.lib.cleanSource ./.;
          cargoLock = { lockFile = ./Cargo.lock; };
        in
        rec {
          default = walbridge;

          walbridge =
            let
              unwrapped = pkgs.rustPlatform.buildRustPackage {
                pname = "walbridge";
                version = "0.1.0";
                src = workspaceSrc;
                inherit cargoLock;
                cargoBuildFlags = [ "-p" "walbridge" ];
                cargoTestFlags = [ "-p" "walbridge" ];
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

          walbridge-extract = pkgs.rustPlatform.buildRustPackage {
            pname = "walbridge-extract";
            version = "0.1.0";
            src = workspaceSrc;
            inherit cargoLock;
            cargoBuildFlags = [ "-p" "walbridge-extract" ];
            cargoTestFlags = [ "-p" "walbridge-extract" ];
            meta.mainProgram = "walbridge-extract";
          };

          walbridge-visualize =
            let
              runtimeLibs = with pkgs; [
                wayland
                libxkbcommon
                libGL
                fontconfig
                vulkan-loader
                libx11
                libxcursor
                libxi
                libxrandr
              ];
              unwrapped = pkgs.rustPlatform.buildRustPackage {
                pname = "walbridge-visualize";
                version = "0.1.0";
                src = workspaceSrc;
                inherit cargoLock;
                cargoBuildFlags = [ "-p" "walbridge-visualize" ];
                cargoTestFlags = [ "-p" "walbridge-visualize" ];
                nativeBuildInputs = [ pkgs.pkg-config ];
                buildInputs = runtimeLibs;
                meta.mainProgram = "walbridge-visualize";
              };
            in
            pkgs.symlinkJoin {
              name = "walbridge-visualize-0.1.0";
              paths = [ unwrapped ];
              nativeBuildInputs = [ pkgs.makeWrapper ];
              postBuild = ''
                wrapProgram $out/bin/walbridge-visualize \
                  --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
              '';
              meta.mainProgram = "walbridge-visualize";
            };
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          runtimeLibs = with pkgs; [
            wayland
            libxkbcommon
            libGL
            fontconfig
            vulkan-loader
            libx11
            libxcursor
            libxi
            libxrandr
          ];
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              rust-analyzer
              pkg-config
            ];
            buildInputs = runtimeLibs ++ (with pkgs; [
              glib
              bat
              adw-gtk3
              gsettings-desktop-schemas
            ]);

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            WALBRIDGE_GTK_BASE_THEME_NAME = "adw-gtk3-dark";
            WALBRIDGE_GTK3_BASE_CSS = "${pkgs.adw-gtk3}/share/themes/adw-gtk3-dark/gtk-3.0/gtk.css";
            WALBRIDGE_GTK4_BASE_CSS = "${pkgs.adw-gtk3}/share/themes/adw-gtk3-dark/gtk-4.0/gtk.css";

            shellHook = ''
              export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
            '';
          };
        }
      );
    };
}
