{
  description = "Floating window terminal multiplexer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      forAllSystems =
        callback:
        nixpkgs.lib.genAttrs [
          "x86_64-linux"
          "aarch64-linux"
        ] (system: callback nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        pkgs:
        let
          inherit (pkgs) lib;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage (finalAttrs: {
            pname = "float-mux";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            meta = {
              description = "Floating window terminal multiplexer";
              changelog = "https://github.com/Henktorius/float/releases/tag/v${finalAttrs.version}";
              homepage = "https://github.com/Henktorius/float";
              license = lib.licenses.mit;
              mainProgram = "float-mux";
              platforms = lib.platforms.linux;
            };
          });
        }
      );
    };
}
