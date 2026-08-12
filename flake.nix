{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    {
      self,
      nixpkgs,
      systems,
    }:
    let
      mapSupportedSystems = nixpkgs.lib.genAttrs (import systems);
      forEachSupportedSystem = f: mapSupportedSystems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forEachSupportedSystem (pkgs: {
        default = pkgs.mkShell {
          buildInputs =
            with pkgs;
            [
              cargo
              rustc
              clippy
              rustfmt
              rust-analyzer
              # WorktreeManager アダプターが git CLI へシェルアウトする(ADR-024)
              git
            ]
            ++ lib.optionals stdenv.isDarwin [ libiconv ];

          env = {
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        };
      });
    };
}
