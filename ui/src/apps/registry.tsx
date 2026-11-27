import { type ComponentType, useEffect, useState } from "react";

import { CodeView } from "../components/CodeView";
import { api } from "../lib/api";
import { errorText } from "../state/actions";
import type { Win } from "../state/shell";
import { Activity } from "./Activity";
import { Approvals } from "./Approvals";
import type { AppId } from "./meta";
import { Profiles } from "./Profiles";
import { SystemApp } from "./SystemApp";
import { Timeline } from "./Timeline";

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
  approvals: Approvals,
  timeline: Timeline,
  system: SystemApp,
  profiles: Profiles,
  activity: Activity,
  viewer: Viewer,
};
