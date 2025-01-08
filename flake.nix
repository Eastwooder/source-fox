{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, pre-commit-hooks, advisory-db }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          # import and bind toolchain to the provided `rust-toolchain.toml` in the root directory
          rustToolchain = (pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml);
          rustNightly = pkgs.pkgsBuildHost.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
          craneLib = ((crane.mkLib pkgs).overrideToolchain rustToolchain);

          # declare the sources
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              # include everything in the `tests` directory - including test objects
              (pkgs.lib.hasInfix "/tests/" path) ||
              # Default filter from crane (allow .rs files)
              (craneLib.filterCargoSources path type)
            ;
          };
          # declare build arguments
          commonArgs = {
            inherit src;
            strictDeps = true;
            buildInputs = with pkgs; [ openssl ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.CoreFoundation
            ];
            nativeBuildInputs = with pkgs; [ pkg-config ];
          };

          # Cargo artifact dependency output
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          individualCrateArgs = commonArgs // {
            inherit cargoArtifacts;
            inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
            # NB: we disable tests since we'll run them all via cargo-nextest
            doCheck = false;
          };

          fileSetForCrate = crate: pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              (craneLib.fileset.commonCargoSources ./github-event-handler)
              (craneLib.fileset.commonCargoSources ./mergeable-compatibility-layer)
              (craneLib.fileset.commonCargoSources ./event-processor)
              (craneLib.fileset.commonCargoSources crate)
            ];
          };

          server = craneLib.buildPackage (individualCrateArgs // {
            pname = "server";
            cargoExtraArgs = "-p server";
            src = fileSetForCrate ./server;
            buildInputs = with pkgs; [ cmake openssl ];
          });

          serverOci = {
            name = "wild-git-yonder";
            tag = "latest";
            config = {
              Cmd = [ "${server}/bin/server" ];
            };
          };
          serverOciImage = pkgs.dockerTools.buildImage ({
            copyToRoot = [ server ];
          } // serverOci);
          serverOciStream = pkgs.dockerTools.streamLayeredImage ({
            contents = [ server ];
          } // serverOci);

          scripts = [
            (pkgs.writeScriptBin "strip-unused-dependencies" ''
              #!${pkgs.zsh}/bin/zsh
              RUSTC=${rustNightly}/bin/rustc ${rustNightly}/bin/cargo udeps --all-targets
            '') # cargo-udeps needs nightly, hence the nightly invocation
          ];
        in
        with pkgs;
        {
          # formatter for the flake.nix
          formatter = nixpkgs-fmt;

          # executes all checks
          checks = {
            inherit server;
            workspace-clippy = craneLib.cargoClippy (commonArgs // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            });
            workspace-doc = craneLib.cargoDoc (commonArgs // {
              inherit cargoArtifacts;
            });
            workspace-fmt = craneLib.cargoFmt {
              inherit src;
            };
            # Run tests with cargo-nextest
            # Consider setting `doCheck = false` on other crate derivations
            # if you do not want the tests to run twice
            workspace-nextest = craneLib.cargoNextest (commonArgs // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
            });
            # Audit dependencies
            my-workspace-audit = craneLib.cargoAudit {
              inherit src advisory-db;
            };
            # Audit licenses
            my-workspace-deny = craneLib.cargoDeny {
              inherit src;
            };
            # pre-commit-checks to be installed for the dev environment
            pre-commit-check = pre-commit-hooks.lib.${system}.run {
              src = ./.;
              # git commit hooks
              hooks = {
                nixpkgs-fmt.enable = true;
                rustfmt.enable = true;
                markdownlint.enable = true;
                commitizen.enable = true;
                typos.enable = true;
              };
            };
          };

          # packages to build and provide
          packages = {
            inherit server;
            server-docker = serverOciImage;
            server-docker-stream = serverOciStream;
            default = server;
            about = pkgs.writeScriptBin "about" ''
              #!/bin/sh
              echo "Welcome to our bot!"
            '';
          };

          # applications which can be started as-is
          apps.server = {
            type = "app";
            program = "${self.packages.${system}.server}/bin/server";
          };

          # development environment provided with all bells and whistles included
          devShells.default = craneLib.devShell {
            inherit (self.checks.${system}.pre-commit-check) shellHook;
            inputsFrom = [
              server
            ];

            # Extra inputs can be added here; cargo and rustc are provided by default.
            packages = with pkgs; [
              act # GitHub Actions runner for locally testing workflows
              cargo-udeps # cargo extension for removing unused dependencies
              cargo-edit # cargo extension for easier management of dependencies in the style of `cargo [rm|upgrade|set-version]`
              cargo-nextest # nextest runner
              cargo-deny # deny licenses
              cargo-audit # audit checks

              bacon # background code checker
            ] ++ scripts;
          };
        });
}
