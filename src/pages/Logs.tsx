import { useEffect, useState } from "react";
import { api, LaunchEvent } from "../lib/tauri";

export function Logs() {
  const [lines, setLines] = useState<LaunchEvent[]>([]);

  useEffect(() => {
    const unlisten = api.onLaunchLog((event) => {
      setLines((prev) => [...prev, event]);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="p-8">
      <h1 className="text-2xl font-bold mb-2">Logs</h1>
      <p className="text-sm text-[var(--color-text-dim)] mb-4">
        Live output from the last launch. Full log files are saved under each profile's{" "}
        <code>logs/</code> folder.
      </p>
      <div className="glass rounded-xl p-4 font-mono text-xs h-[60vh] overflow-y-auto">
        {lines.length === 0 && (
          <div className="text-[var(--color-text-dim)]">No launch output yet — press PLAY on Home.</div>
        )}
        {lines.map((line, i) => (
          <div key={i} className={line.stream === "stderr" ? "text-[var(--color-danger)]" : ""}>
            {line.line}
          </div>
        ))}
      </div>
    </div>
  );
}
