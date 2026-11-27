{ lib, rustPlatform }:

rustPlatform.buildRustPackage {
  pname = "fluxd";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
      ../profiles
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;
  # flux-session runs as the console user and serves the UI's terminal, files and apps.
  cargoBuildFlags = [ "-p" "fluxd" "-p" "flux-session" ];
  doCheck = false;

  meta = {
    description = "FluxOS system daemon: policy engine, generations, applier, health checks and audit log";
    license = lib.licenses.mit;
    mainProgram = "fluxd";
    platforms = lib.platforms.linux;
  };
}
