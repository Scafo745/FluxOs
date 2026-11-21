import { useEffect, useState } from "react";

import { liveWallpaper, useSystem } from "../state/system";

/** fluxd's generated wallpaper, cross-faded whenever a generation changes it. */
export function Wallpaper() {
  const key = useSystem((s) => s.themeKey);
  const previewUrl = useSystem((s) => s.preview?.wallpaperUrl);
  const url = previewUrl ?? liveWallpaper(key);
  const [layers, setLayers] = useState<{ url: string; id: number }[]>([]);

  useEffect(() => {
    let live = true;
    const img = new Image();
    img.onload = () => {
      if (live) setLayers((ls) => [...ls.filter((l) => l.url !== url).slice(-1), { url, id: Date.now() }]);
    };
    img.src = url;
    return () => {
      live = false;
    };
  }, [url]);

  return (
    <div className="absolute inset-0 overflow-hidden bg-bg" aria-hidden="true">
      {layers.map((l) => (
        <img key={l.id} src={l.url} alt="" draggable={false} className="wallpaper-layer absolute inset-0 h-full w-full object-cover" />
      ))}
      <div
        className="pointer-events-none absolute inset-0"
        style={{ background: "radial-gradient(ellipse at center, transparent 55%, color-mix(in oklab, var(--flux-color-background) 50%, transparent))" }}
      />
    </div>
  );
}
