/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Base URL of fluxd when the UI is not served from it (default: same origin). */
  readonly VITE_FLUXD_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
