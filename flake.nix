{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    # The version of wasm-bindgen-cli needs to match the version in Cargo.lock
    # Update this to include the version you need
    nixpkgs-for-wasm-bindgen.url = "github:NixOS/nixpkgs/4e6868b1aa3766ab1de169922bb3826143941973";
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane, nixpkgs-for-wasm-bindgen, pre-commit-hooks }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
            # config.allowUnfreePredicate = pkg: builtins.elem (nixpkgs.lib.getName pkg) [
            #   # "vscode-with-extensions-1.86.0"
            #   "vscode-extension-ms-vscode-remote-remote-containers-0.305.0"
            # ];
            config.allowUnfree = true;
          };

          # import and bind toolchain to the provided `rust-toolchain.toml` in the root directory
          rustToolchain = (pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
            # Set the build targets supported by the toolchain, wasm32-unknown-unknown is required for trunk.
            extensions = [
              "rust-src" # needed by rust-analyzer vscode extension when installed through internet
              "rust-analyzer"
            ];
            targets = [ "wasm32-unknown-unknown" ];
          };
          craneLib = ((crane.mkLib pkgs).overrideToolchain rustToolchain).overrideScope (_final: _prev: {
            # The version of wasm-bindgen-cli needs to match the version in Cargo.lock. You
            # can unpin this if your nixpkgs commit contains the appropriate wasm-bindgen-cli version
            inherit (import nixpkgs-for-wasm-bindgen { inherit system; }) wasm-bindgen-cli;
          });

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
          wasmSrc = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              # include all html and scss files
              (pkgs.lib.hasSuffix "\.html" path) ||
              (pkgs.lib.hasSuffix "\.scss" path) ||
              # Include additional assets we may have
              (pkgs.lib.hasInfix "/assets/" path) ||
              # Default filter from crane (allow .rs files)
              (craneLib.filterCargoSources path type)
            ;
          };

          # declare the build inputs used to build the projects
          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
            act
          ] ++ macosBuildInputs;
          # declare the build inputs used to link and run the projects, will be included in the final artifact container
          buildInputs = with pkgs; [ openssl sqlite ];
          macosBuildInputs = with pkgs.darwin.apple_sdk.frameworks;
            [ ]
            ++ (nixpkgs.lib.optionals (nixpkgs.lib.hasSuffix "-darwin" system) [
              Security
              CoreFoundation
            ]);

          # declare build arguments
          commonArgs = {
            inherit src buildInputs nativeBuildInputs;
          };
          wasmArgs = {
            src = wasmSrc;
            pname = "wasm-client";
            cargoExtraArgs = "-p service-ui";
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          };

          # Cargo artifact dependency output
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          cargoArtifactsWasm = craneLib.buildDepsOnly (wasmArgs // {
            doCheck = false;
          });

          # Actual targets to be built and executed
          service-ui = craneLib.buildTrunkPackage (wasmArgs // {
            inherit cargoArtifactsWasm;
            pname = "service-ui";
            cargoExtraArgs = "-p service-ui";
            trunkIndexPath = "service-ui/index.html";
            # The version of wasm-bindgen-cli here must match the one from Cargo.lock.
            wasm-bindgen-cli = pkgs.wasm-bindgen-cli.override {
              version = "0.2.90";
              hash = "sha256-X8+DVX7dmKh7BgXqP7Fp0smhup5OO8eWEhn26ODYbkQ=";
              cargoHash = "sha256-ckJxAR20GuVGstzXzIj1M0WBFj5eJjrO2/DRMUK5dwM=";
            };
          });

          server = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            pname = "server";
            cargoExtraArgs = "-p server";
            buildInputs = [ pkgs.cmake pkgs.openssl ];
            CLIENT_DIST = service-ui;
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
        in
        with pkgs;
        {
          # formatter for the flake.nix
          formatter = nixpkgs-fmt;

          # executes all checks
          checks = {
            inherit server service-ui;
            wild-git-yonder-clippy = craneLib.cargoClippy (commonArgs // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets";
              CLIENT_DIST = "";
            });
            wild-git-yonder-fmt = craneLib.cargoFmt commonArgs;
            # pre-commit-checks to be installed for the dev environment
            pre-commit-check = pre-commit-hooks.lib.${system}.run {
              src = ./.;

              # git commit hooks
              hooks = {
                nixpkgs-fmt.enable = true;
                # clippy.enable = true;
                rustfmt.enable = true;
                markdownlint.enable = true;
                commitizen.enable = true;
                typos.enable = true;
              };
            };
          };

          # packages to build and provide
          packages = {
            inherit server service-ui;
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
              service-ui
            ];
            # used by codium - pins the rust-analyzer version to this project's rust version
            RUST_ANALYZER_SERVER_PATH = "${rustToolchain}/bin/rust-analyzer";
            packages = with pkgs; [
              (vscode-with-extensions.override {
                vscode = vscodium;
                vscodeExtensions = with vscode-extensions; [
                  # direnv/nix related
                  mkhl.direnv
                  jnoortheen.nix-ide
                  arrterian.nix-env-selector

                  # rust related plugins
                  rust-lang.rust-analyzer
                  serayuzgur.crates
                  # dustypomerleau.rust-syntax

                  # unfree, but helpful stuff
                  ms-vscode-remote.remote-containers
                  ms-azuretools.vscode-docker
                ] ++ vscode-utils.extensionsFromVscodeMarketplace [
                  # {
                  #   name = "vscode-lldb";
                  #   publisher = "vadimcn";
                  #   version = "1.10.0";
                  #   sha256 = "sha256-RAKv7ESw0HG/avBOPE1CTr0THsB7UWx0haJVd/Dm9Gg=";
                  # }
                  {
                    name = "intellij-idea-keybindings";
                    publisher = "k--kato";
                    version = "1.5.13";
                    sha256 = "sha256-IewHupCMUIgtMLcMV86O0Q1n/0iUgknHMR8m8fci5OU=";
                  }
                ];
              })
            ];
          };
        });
}
