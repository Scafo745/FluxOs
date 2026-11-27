{ lib, buildNpmPackage, importNpmLock, nodejs_22 }:

# Static build of the FluxOS web shell. fluxd serves it with `--ui-dir`.
buildNpmPackage {
  pname = "flux-ui";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ../ui;
    fileset = lib.fileset.unions [
      ../ui/package.json
      ../ui/package-lock.json
      ../ui/tsconfig.json
      ../ui/vite.config.ts
      ../ui/index.html
      ../ui/public
      ../ui/src
    ];
  };

  nodejs = nodejs_22;
  # Fetches each dependency by the integrity hash in package-lock.json.
  npmDeps = importNpmLock { npmRoot = ../ui; };
  npmConfigHook = importNpmLock.npmConfigHook;

  # `npm run build` type-checks, then writes dist/. Only the static files ship.
  installPhase = ''
    runHook preInstall
    cp -r dist $out
    runHook postInstall
  '';

  meta = {
    description = "FluxOS web shell: boot screen, pairing, desktop, AI assistant and admin console";
    # FluxOS and React are MIT, lucide is ISC, the bundled Fontsource fonts are OFL 1.1.
    license = with lib.licenses; [ mit isc ofl ];
    platforms = lib.platforms.all;
  };
}
