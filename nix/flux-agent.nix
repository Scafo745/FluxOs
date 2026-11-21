{ lib, buildNpmPackage, importNpmLock, nodejs_22 }:

buildNpmPackage {
  pname = "flux-agent";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../agent;
    fileset = lib.fileset.unions [
      ../agent/package.json
      ../agent/package-lock.json
      ../agent/tsconfig.json
      ../agent/src
      ../agent/knowledge
    ];
  };

  nodejs = nodejs_22;
  # Fetches each dependency by the integrity hash in package-lock.json.
  npmDeps = importNpmLock { npmRoot = ../agent; };
  npmConfigHook = importNpmLock.npmConfigHook;

  meta = {
    description = "FluxOS AI orchestrator: natural-language requests to validated plans";
    license = lib.licenses.mit;
    mainProgram = "flux-agent";
    platforms = lib.platforms.linux;
  };
}
