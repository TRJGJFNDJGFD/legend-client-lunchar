import { useEffect, useState } from "react";
import { api, Account, AuthCodeEvent } from "../lib/tauri";

export function Accounts() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [status, setStatus] = useState<"idle" | "waiting-code" | "polling" | "error">("idle");
  const [deviceCode, setDeviceCode] = useState<AuthCodeEvent | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setAccounts(await api.listAccounts());
  }

  useEffect(() => {
    refresh();

    const unlistenCode = api.onAuthDeviceCode((event) => {
      setDeviceCode(event);
      setStatus("polling");
    });
    const unlistenResult = api.onAuthResult((event) => {
      if (event.success) {
        setStatus("idle");
        setDeviceCode(null);
        refresh();
      } else {
        setStatus("error");
        setError(event.error ?? "Unknown error");
      }
    });

    return () => {
      unlistenCode.then((fn) => fn());
      unlistenResult.then((fn) => fn());
    };
  }, []);

  async function handleSignIn() {
    setStatus("waiting-code");
    setError(null);
    await api.startLogin();
  }

  async function copyCode() {
    if (deviceCode) {
      await navigator.clipboard.writeText(deviceCode.user_code);
    }
  }

  return (
    <div className="p-8 max-w-xl">
      <h1 className="text-2xl font-bold mb-6">Accounts</h1>

      {accounts.length === 0 && status === "idle" && (
        <div className="glass rounded-xl p-5 mb-4">
          <p className="text-sm text-[var(--color-text-dim)] mb-4">
            No Microsoft account signed in yet.
          </p>
          <button
            onClick={handleSignIn}
            className="px-5 py-2 rounded-lg bg-[var(--color-primary)] text-black text-sm font-semibold"
          >
            Sign in with Microsoft
          </button>
        </div>
      )}

      {(status === "waiting-code" || status === "polling") && (
        <div className="glass rounded-xl p-5 mb-4 text-center">
          {status === "waiting-code" && (
            <p className="text-sm text-[var(--color-text-dim)]">Requesting a sign-in code…</p>
          )}
          {status === "polling" && deviceCode && (
            <>
              <p className="text-sm text-[var(--color-text-dim)] mb-3">
                Go to <span className="text-[var(--color-primary)]">{deviceCode.verification_uri}</span> and
                enter this code:
              </p>
              <div className="flex items-center justify-center gap-3">
                <div className="text-3xl font-mono font-bold tracking-widest bg-[var(--color-panel-2)] rounded-lg px-6 py-3">
                  {deviceCode.user_code}
                </div>
                <button
                  onClick={copyCode}
                  className="px-3 py-2 rounded-lg bg-[var(--color-panel-2)] text-sm"
                >
                  Copy
                </button>
              </div>
              <p className="text-xs text-[var(--color-text-dim)] mt-4">
                Waiting for you to complete sign-in in your browser…
              </p>
            </>
          )}
        </div>
      )}

      {status === "error" && (
        <div className="glass rounded-xl p-5 mb-4 text-sm text-[var(--color-danger)]">
          Sign-in failed: {error}
          <button
            onClick={handleSignIn}
            className="block mt-3 px-4 py-1.5 rounded-lg bg-[var(--color-panel-2)] text-[var(--color-text)] text-sm"
          >
            Try again
          </button>
        </div>
      )}

      <div className="flex flex-col gap-2">
        {accounts.map((account) => (
          <div
            key={account.id}
            className={`glass rounded-xl px-4 py-3 flex items-center justify-between ${
              account.is_active ? "border-[var(--color-primary)]" : ""
            }`}
          >
            <div className="flex items-center gap-3">
              {account.skin_url && (
                <img src={account.skin_url} alt="" className="w-8 h-8 rounded" />
              )}
              <div>
                <div className="font-semibold text-sm">{account.username}</div>
                <div className="text-xs text-[var(--color-text-dim)]">
                  {account.is_active ? "Active" : "Signed in"}
                </div>
              </div>
            </div>
            <div className="flex gap-2">
              {!account.is_active && (
                <button
                  onClick={() => api.setActiveAccount(account.id).then(refresh)}
                  className="text-xs text-[var(--color-primary)] hover:underline"
                >
                  Use this account
                </button>
              )}
              <button
                onClick={() => api.removeAccount(account.id).then(refresh)}
                className="text-xs text-[var(--color-danger)] hover:underline"
              >
                Remove
              </button>
            </div>
          </div>
        ))}
      </div>

      {accounts.length > 0 && status === "idle" && (
        <button
          onClick={handleSignIn}
          className="mt-4 px-4 py-2 rounded-lg bg-[var(--color-panel-2)] text-sm"
        >
          + Add another account
        </button>
      )}
    </div>
  );
}
