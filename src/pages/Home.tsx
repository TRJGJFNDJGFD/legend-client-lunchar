import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useAppStore } from "../store/useAppStore";
import { api, LaunchEvent } from "../lib/tauri";

export function Home() {
  const { javaInstalls, javaChecked, profiles, activeProfileId, refreshJava, refreshProfiles } =
    useAppStore();
  const [launching, setLaunching] = useState(false);
  const [lastLines, setLastLines] = useState<string[]>([]);
  const [launchError, setLaunchError] = useState<string | null>(null);

  useEffect(() => {
    refreshJava();
    refreshProfiles();
  }, [refreshJava, refreshProfiles]);

  useEffect(() => {
    const unlisten = api.onLaunchLog((event: LaunchEvent) => {
      setLastLines((prev) => [...prev.slice(-6), event.line]);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? profiles[0];

  async function handlePlay() {
    if (!activeProfile) return;
    setLaunching(true);
    setLaunchError(null);
    setLastLines([]);
    try {
      const result = await api.launchProfile(activeProfile.id);
      if (result.crashed) {
        setLaunchError(`Process exited with code ${result.exit_code ?? "unknown"}.`);
      }
    } catch (err) {
      setLaunchError(String(err));
    } finally {
      setLaunching(false);
    }
  }

  return (
    <div className="h-full flex flex-col items-center justify-center gap-8 px-8">
      <div className="text-center">
        <h1 className="text-5xl font-extrabold tracking-tight">
          Legend<span className="text-[var(--color-primary)]">Client</span>
        </h1>
        <p className="text-[var(--color-text-dim)] mt-2">Your Minecraft. Your way.</p>
      </div>

      {javaChecked && javaInstalls.length === 0 && (
        <div className="glass rounded-xl px-5 py-4 text-sm text-[var(--color-danger)] max-w-md text-center">
          No Java installation was found on this system. Install a Java runtime to continue.
        </div>
      )}

      {profiles.length === 0 ? (
        <div className="glass rounded-xl px-6 py-5 text-center max-w-md">
          <p className="text-sm text-[var(--color-text-dim)] mb-3">
            You don't have a profile yet — create one to play.
          </p>
          <Link
            to="/profiles"
            className="inline-block px-5 py-2 rounded-full bg-[var(--color-primary)] text-black font-semibold text-sm"
          >
            Create your first profile
          </Link>
        </div>
      ) : (
        <div className="flex flex-col items-center gap-4">
          <button
            onClick={handlePlay}
            disabled={launching || javaInstalls.length === 0}
            className="px-14 py-4 rounded-full bg-gradient-to-r from-[var(--color-primary)] to-[var(--color-accent)] text-black font-bold text-lg shadow-lg shadow-[var(--color-primary)]/20 disabled:opacity-50 transition-transform hover:scale-[1.02]"
          >
            {launching ? "LAUNCHING…" : "PLAY"}
          </button>

          <div className="flex items-center gap-3 text-sm text-[var(--color-text-dim)]">
            <span className="glass px-3 py-1 rounded-full">{activeProfile?.minecraft_version}</span>
            <span className="glass px-3 py-1 rounded-full capitalize">{activeProfile?.loader}</span>
            <span className="glass px-3 py-1 rounded-full">{activeProfile?.name}</span>
          </div>
        </div>
      )}

      {launchError && (
        <div className="glass rounded-xl px-5 py-3 text-sm text-[var(--color-danger)] max-w-md text-center">
          {launchError}
        </div>
      )}

      {lastLines.length > 0 && (
        <div className="glass rounded-xl px-4 py-3 max-w-lg w-full text-xs font-mono text-[var(--color-text-dim)] space-y-0.5">
          {lastLines.map((line, i) => (
            <div key={i} className="truncate">
              {line}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
