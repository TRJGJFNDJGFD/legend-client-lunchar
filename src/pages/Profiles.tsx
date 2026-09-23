import { useEffect, useState } from "react";
import { useAppStore } from "../store/useAppStore";

const LOADERS = ["vanilla", "fabric", "forge", "neoforge"];

export function Profiles() {
  const { profiles, refreshProfiles, createProfile, deleteProfile, activeProfileId, setActiveProfile } =
    useAppStore();
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState("");
  const [version, setVersion] = useState("1.21.1");
  const [loader, setLoader] = useState("fabric");
  const [minRam, setMinRam] = useState(2048);
  const [maxRam, setMaxRam] = useState(4096);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    refreshProfiles();
  }, [refreshProfiles]);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    setBusy(true);
    try {
      await createProfile({
        name,
        minecraft_version: version,
        loader,
        min_ram_mb: minRam,
        max_ram_mb: maxRam,
      });
      setShowForm(false);
      setName("");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="p-8 max-w-3xl">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold">Profiles</h1>
        <button
          onClick={() => setShowForm((v) => !v)}
          className="px-4 py-2 rounded-lg bg-[var(--color-primary)] text-black text-sm font-semibold"
        >
          {showForm ? "Cancel" : "+ New Profile"}
        </button>
      </div>

      {showForm && (
        <form onSubmit={handleCreate} className="glass rounded-xl p-5 mb-6 flex flex-col gap-3">
          <input
            className="bg-[var(--color-panel-2)] rounded-lg px-3 py-2 text-sm outline-none"
            placeholder="Profile name (e.g. Crystal PvP)"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
          />
          <div className="flex gap-3">
            <input
              className="bg-[var(--color-panel-2)] rounded-lg px-3 py-2 text-sm outline-none flex-1"
              placeholder="Minecraft version"
              value={version}
              onChange={(e) => setVersion(e.target.value)}
            />
            <select
              className="bg-[var(--color-panel-2)] rounded-lg px-3 py-2 text-sm outline-none flex-1"
              value={loader}
              onChange={(e) => setLoader(e.target.value)}
            >
              {LOADERS.map((l) => (
                <option key={l} value={l}>
                  {l}
                </option>
              ))}
            </select>
          </div>
          <div className="flex gap-3 items-center text-sm text-[var(--color-text-dim)]">
            <label className="flex items-center gap-2">
              Min RAM (MB)
              <input
                type="number"
                step={512}
                className="bg-[var(--color-panel-2)] rounded-lg px-2 py-1 w-24 outline-none"
                value={minRam}
                onChange={(e) => setMinRam(Number(e.target.value))}
              />
            </label>
            <label className="flex items-center gap-2">
              Max RAM (MB)
              <input
                type="number"
                step={512}
                className="bg-[var(--color-panel-2)] rounded-lg px-2 py-1 w-24 outline-none"
                value={maxRam}
                onChange={(e) => setMaxRam(Number(e.target.value))}
              />
            </label>
          </div>
          <button
            type="submit"
            disabled={busy}
            className="self-end px-4 py-2 rounded-lg bg-[var(--color-success)] text-black text-sm font-semibold disabled:opacity-50"
          >
            {busy ? "Creating…" : "Create Profile"}
          </button>
        </form>
      )}

      <div className="flex flex-col gap-2">
        {profiles.length === 0 && (
          <p className="text-sm text-[var(--color-text-dim)]">No profiles yet.</p>
        )}
        {profiles.map((p) => (
          <div
            key={p.id}
            className={`glass rounded-xl px-4 py-3 flex items-center justify-between cursor-pointer ${
              p.id === activeProfileId ? "border-[var(--color-primary)]" : ""
            }`}
            onClick={() => setActiveProfile(p.id)}
          >
            <div>
              <div className="font-semibold text-sm">{p.name}</div>
              <div className="text-xs text-[var(--color-text-dim)]">
                {p.minecraft_version} · {p.loader} · {p.min_ram_mb}–{p.max_ram_mb} MB
              </div>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                deleteProfile(p.id);
              }}
              className="text-xs text-[var(--color-danger)] hover:underline"
            >
              Delete
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
