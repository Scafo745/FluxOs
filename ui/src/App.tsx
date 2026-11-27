import { useCallback, useState } from "react";

import { InitialBoot } from "./boot/InitialBoot";
import { Desktop } from "./desktop/Desktop";
import { ThemeRuntime } from "./desktop/ThemeRuntime";
import { PairScreen } from "./pair/PairScreen";
import { useSession } from "./state/session";

export function App() {
  const session = useSession((s) => s.session);
  const [booted, setBooted] = useState(false);
  const done = useCallback(() => setBooted(true), []);

  return (
    <>
      <ThemeRuntime />
      {!booted ? <InitialBoot onDone={done} /> : session ? <Desktop key={session.sessionId} /> : <PairScreen />}
    </>
  );
}
