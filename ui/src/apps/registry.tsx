import { type ComponentType, lazy, useEffect, useState } from "react";

import { CodeView } from "../components/CodeView";
import { api } from "../lib/api";
import { errorText } from "../state/actions";
import type { Win } from "../state/shell";
import { Activity } from "./Activity";
import { Approvals } from "./Approvals";
import { Files } from "./Files";
import type { AppId } from "./meta";
import { Profiles } from "./Profiles";
import { Security } from "./Security";
import { Synthesizer } from "./Synthesizer";
import { SystemApp } from "./SystemApp";
import { Tasks } from "./Tasks";
import { Timeline } from "./Timeline";

// xterm.js is large; load it with the first terminal window.
const Terminal = lazy(() => import("./Terminal").then((m) => ({ default: m.Terminal })));

/** A file rendered for one generation. */
function Viewer({ win }: { win: Win }) {
  const generation = Number(win.props.generation);
  const path = String(win.props.path ?? "etc/nixos/fluxos/generated.nix");
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api
      .generationFile(generation, path)
      .then(setText)
      .catch((e) => setError(errorText(e)));
  }, [generation, path]);
  return <CodeView text={text} path={path} error={error} />;
}

export const APP_COMPONENTS: Record<AppId, ComponentType<{ win: Win }>> = {
  terminal: Terminal,
  files: Files,
  tasks: Tasks,
  security: Security,
  synthesizer: Synthesizer,
  approvals: Approvals,
  timeline: Timeline,
  system: SystemApp,
  profiles: Profiles,
  activity: Activity,
  viewer: Viewer,
};
