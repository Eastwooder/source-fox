# Wild Git Yonder (WGY)

[![Check](https://github.com/Eastwooder/wild-git-yonder/actions/workflows/check.yaml/badge.svg)](https://github.com/Eastwooder/wild-git-yonder/actions/workflows/check.yaml)

A GitHub Application meant to watch over your source code.

## Getting started

Either you install all required packages and tools, or you just use the
provided developer environment with `nix develop`.

### VSCode / Codium

There is an embedded development environment in form of VSCode
(in this case Codium) which has all necessary extensions pre-configured.
This should help getting started without any kind of setup.

If you're not using `direnv` just run `nix develop -c codium .`,
otherwise `codium` will be on the path and `codium .` will suffice.

## Running the server

To run the server your can just execute `nix run .#server`.

## OCI Image

You can either build the docker container as a single layer or stream all layers.

- single layer

  ```shell
  nix build .#server-docker && zcat ./result | docker load
  ```

- multiple layers

  ```shell
  nix build .#server-docker-stream && ./result | docker load
  ```
