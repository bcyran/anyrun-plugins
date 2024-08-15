{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    advisory-db,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
      };

      inherit (pkgs) lib;

      craneLib = crane.mkLib nixpkgs.legacyPackages.${system};
      src = craneLib.cleanCargoSource (craneLib.path ./.);

      commonArgs = {
        inherit src;
        buildInputs = with pkgs;
          [
            pkg-config
          ]
          ++ lib.optionals pkgs.stdenv.isDarwin [];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      anyrun-powermenu = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
        });
    in {
      checks = {
        inherit anyrun-powermenu;

        anyrun-powermenu-clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = nixpkgs.lib.strings.concatStringsSep " " [
              "--all-targets"
              "--"
              "-Dclippy::correctness"
              "-Dclippy::nursery"
              "-Aclippy::option_if_let_else"
              "-Dclippy::pedantic"
              "-Aclippy::module_name_repetitions"
              "-Dclippy::perf"
              "-Dclippy::suspicious"
              "-Dclippy::style"
            ];
          });

        anyrun-powermenu-doc = craneLib.cargoDoc (commonArgs
          // {
            inherit cargoArtifacts;
          });

        anyrun-powermenu-fmt = craneLib.cargoFmt {
          inherit src;
        };

        anyrun-powermenu-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };
      };

      packages.default = anyrun-powermenu;

      formatter = pkgs.alejandra;

      devShells.default = pkgs.mkShell {
        inputsFrom = builtins.attrValues self.checks.${system};

        nativeBuildInputs = with pkgs; [
          cargo # rust package manager
          clippy # opinionated rust formatter
          rustc # rust compiler
          rustfmt # rust formatter
          rust-analyzer # rust analyzer

          marksman # markdown LSP
          markdownlint-cli2 # markdwon linter
        ];
      };
    });
}
