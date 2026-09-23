import { useEffect, useState } from "react";
import { useAppStore } from "../store/useAppStore";
import { api, ModSearchResult, ModVersion } from "../lib/tauri";

export function Mods() {
  const { profiles, activeProfileId } = useAppStore();
  const activeProfile = profiles.find((p) => p.id === activeProfileId) ?? profiles[0];

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModSearchResult[]>([]);
  const [installed, setInstalled] = useState<string[]>([]);
  const [searching, setSearching] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (activeProfile) refreshInstalled();
  }, [activeProfile?.id]);

  async function refreshInstalled() {
    if (!activeProfile) return;
    setInstalled(await api.listInstalledMods(activeProfile.id));
  }

  async function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    if (!activeProfile) return;
    setSearching(true);
    setError(null);
    try {
      const hits = await api.searchMods(query, activeProfile.minecraft_version, activeProfile.loader);
      setResults(hits);
    } catch (err) {
      setError(String(err));
    } finally {
      setSearching(false);
    }
  }

  async function handleInstall(mod: ModSearchResult) {
    if (!activeProfile) return;
    setInstallingId(mod.project_id);
    setError(null);
    try {
      const versions: ModVersion[] = await api.getModVersions(
        mod.project_id,
        activeProfile.minecraft_version,
        activeProfile.loader,
      );
      const best = versions[0];
      if (!best) {
        setError(`No version of ${mod.title} is compatible with ${activeProfile.minecraft_version} (${activeProfile.loader}).`);
        return;
      }

      await api.installModWithDependencies({
        profileId: activeProfile.id,
        projectId: mod.project_id,
        versionId: best.version_id,
        minecraftVersion: activeProfile.minecraft_version,
        loader: activeProfile.loader,
      });
      await refreshInstalled();
    } catch (err) {
      setError(
        `Failed to install ${mod.title}: ${String(err)}`,
      );
    } finally {
      setInstallingId(null);
    }
  }

  async function handleRemove(filename: string) {
    if (!activeProfile) return;
    await api.removeInstalledMod(activeProfile.id, filename);
    await refreshInstalled();
  }

  if (!activeProfile) {
    return (
      <div className="p-8">
        <h1 className="text-2xl font-bold mb-2">Mods</h1>
        <p className="text-sm text-[var(--color-text-dim)]">
          Create a profile first — mods install into a specific profile's folder.
        </p>
      </div>
    );
  }

  return (
    <div className="p-8 max-w-3xl">
      <div className="flex items-center justify-between mb-2">
        <h1 className="text-2xl font-bold">Mods</h1>
        <span className="text-xs text-[var(--color-text-dim)]">
          Installing into: <strong>{activeProfile.name}</strong> ({activeProfile.minecraft_version},{" "}
          {activeProfile.loader})
        </span>
      </div>
      <p className="text-xs text-[var(--color-text-dim)] mb-4">
        Searches Modrinth. Downloads are hash-verified, and required dependencies install automatically.
      </p>

      <form onSubmit={handleSearch} className="flex gap-2 mb-6">
        <input
          className="bg-[var(--color-panel-2)] rounded-lg px-3 py-2 text-sm outline-none flex-1"
          placeholder="Search mods (e.g. sodium)"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button
          type="submit"
          disabled={searching}
          className="px-4 py-2 rounded-lg bg-[var(--color-primary)] text-black text-sm font-semibold disabled:opacity-50"
        >
          {searching ? "Searching…" : "Search"}
        </button>
      </form>

      {error && (
        <div className="glass rounded-xl px-4 py-3 mb-4 text-sm text-[var(--color-danger)]">{error}</div>
      )}

      {installed.length > 0 && (
        <div className="mb-6">
          <h2 className="text-sm font-semibold text-[var(--color-text-dim)] uppercase mb-2">
            Installed in this profile
          </h2>
          <div className="flex flex-col gap-1">
            {installed.map((filename) => (
              <div key={filename} className="glass rounded-lg px-3 py-2 flex justify-between text-sm">
                <span className="font-mono text-xs">{filename}</span>
                <button
                  onClick={() => handleRemove(filename)}
                  className="text-xs text-[var(--color-danger)] hover:underline"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="flex flex-col gap-2">
        {results.map((mod) => (
          <div key={mod.project_id} className="glass rounded-xl px-4 py-3 flex items-center gap-4">
            {mod.icon_url && <img src={mod.icon_url} alt="" className="w-10 h-10 rounded-lg" />}
            <div className="flex-1">
              <div className="font-semibold text-sm">{mod.title}</div>
              <div className="text-xs text-[var(--color-text-dim)]">
                by {mod.author} · {mod.downloads.toLocaleString()} downloads
              </div>
              <div className="text-xs text-[var(--color-text-dim)] mt-1 line-clamp-1">{mod.description}</div>
            </div>
            <button
              onClick={() => handleInstall(mod)}
              disabled={installingId === mod.project_id}
              className="px-3 py-1.5 rounded-lg bg-[var(--color-success)] text-black text-xs font-semibold disabled:opacity-50 shrink-0"
            >
              {installingId === mod.project_id ? "Installing…" : "Install"}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
