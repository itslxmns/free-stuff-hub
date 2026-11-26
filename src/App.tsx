import { useEffect, useState } from "react";
import "./App.css";
import { api, isTauri, type IdleView, type Settings, type UpdateView, type Snapshot } from "./lib/api";
import { Icon, type IconName } from "./lib/icons";
import { FreeGames } from "./modules/FreeGames";
import { SteamIdle } from "./modules/SteamIdle";
import { PLANNED, PlannedPage } from "./modules/Planned";

type Page = "free-games" | "steam-idle" | "log" | "settings" | (typeof PLANNED)[number]["id"];

export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [page, setPage] = useState<Page>("free-games");
  const [error, setError] = useState<string | null>(null);
  const [idle, setIdle] = useState<IdleView | null>(null);
  const [update, setUpdate] = useState<UpdateView | null>(null);

  useEffect(() => {
    api.snapshot().then(setSnap).catch((e) => setError(String(e)));
    api.idleState().then(setIdle).catch(() => {});
    api.updateState().then(setUpdate).catch(() => {});
    const unUpdate = api.onUpdate(setUpdate);
    const un = api.onSnapshot(setSnap);
    const unIdle = api.onIdle(setIdle);
    return () => {
      un.then((f) => f());
      unIdle.then((f) => f());
      unUpdate.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!error) return;
    const t = setTimeout(() => setError(null), 6000);
    return () => clearTimeout(t);
  }, [error]);

  if (!snap) return <div className="loading">Loading...</div>;

  const saveSettings = (s: Settings) => api.updateSettings(s).then(setSnap).catch((e) => setError(String(e)));
  const freeCount = snap.games.filter((g) => g.status !== "upcoming" && !g.claimed).length;
  const planned = PLANNED.find((m) => m.id === page);

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="logo"><Icon name="tag" size={18} /></span>
          <div className="brand-name">Free Stuff Hub</div>
        </div>
        <nav>
          <div className="nav-label">Modules</div>
          <NavItem icon="gamepad" label="Free games" active={page === "free-games"} onClick={() => setPage("free-games")} count={freeCount} />
          <NavItem
            icon="cards"
            label="Steam card idling"
            active={page === "steam-idle"}
            onClick={() => setPage("steam-idle")}
            tag={idle?.running ? "On" : undefined}
          />
          {PLANNED.map((m) => (
            <NavItem key={m.id} icon={m.icon} label={m.name} active={page === m.id} onClick={() => setPage(m.id)} tag="Soon" />
          ))}
          <div className="nav-label">App</div>
          <NavItem icon="log" label="Activity log" active={page === "log"} onClick={() => setPage("log")} />
          <NavItem icon="settings" label="Settings" active={page === "settings"} onClick={() => setPage("settings")} />
        </nav>
        <div className="sidebar-foot">Closing the window keeps checking from the tray</div>
      </aside>

      <main className="content">
        {!isTauri && <div className="demo-bar">Browser preview with sample data. Run <code>npm run tauri dev</code> for the real app</div>}
        {update && <UpdateBanner u={update} onError={setError} />}
        {page === "free-games" && <FreeGames snap={snap} onSnap={setSnap} onError={setError} />}
        {page === "steam-idle" && (
          <SteamIdle settings={snap.settings} onSaveSettings={saveSettings} onError={setError} idle={idle} onIdle={setIdle} />
        )}
        {planned && <PlannedPage m={planned} />}
        {page === "log" && <ActivityLog snap={snap} />}
        {page === "settings" && <SettingsPage s={snap.settings} onSave={saveSettings} update={update} onUpdate={setUpdate} onError={setError} />}
      </main>

      {!snap.settings.acknowledgedRisk && (
        <div className="modal-backdrop">
          <div className="modal">
            <h2>Before you start</h2>
            <p>
              Free Stuff Hub automates reward farming. Two planned modules, <em>Discord quests</em> and{" "}
              <em>Twitch and Kick drops</em>, break those platforms' Terms of Service and can get rewards revoked or
              accounts flagged. They'll ship with safeguards like random timing and one task at a time, but the risk is
              yours.
            </p>
            <p>
              The app never asks for your passwords or tokens. You always sign in on the official site, inside an app
              window or your browser.
            </p>
            <button className="btn primary" onClick={() => saveSettings({ ...snap.settings, acknowledgedRisk: true })}>
              I understand the risks
            </button>
          </div>
        </div>
      )}

      {error && (
        <div className="toast" role="alert" onClick={() => setError(null)}>
          <span className="toast-dot" />
          {error}
        </div>
      )}
    </div>
  );
}

function NavItem(props: {
  icon: IconName;
  label: string;
  active: boolean;
  onClick: () => void;
  count?: number;
  tag?: string;
}) {
  return (
    <button className={`nav-item ${props.active ? "active" : ""}`} aria-current={props.active ? "page" : undefined} onClick={props.onClick}>
      <span className="nav-icon"><Icon name={props.icon} /></span>
      <span className="nav-text">{props.label}</span>
      {!!props.count && <span className="nav-meta">{props.count}</span>}
      {props.tag && <span className="nav-meta">{props.tag}</span>}
    </button>
  );
}

function ActivityLog({ snap }: { snap: Snapshot }) {
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Activity log</h1>
          <p className="page-sub">Checks, new offers and claimed rewards, newest first</p>
        </div>
      </header>
      <div className="panel log">
        {snap.log.length === 0 && <p className="empty">No activity yet</p>}
        {snap.log.map((l, i) => (
          <div key={i} className={`log-row lvl-${l.level}`}>
            <span className="log-time">
              {new Date(l.at).toLocaleString("en-GB", { day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit" })}
            </span>
            <span className="log-mod">{l.module}</span>
            <span className="log-msg">{l.message}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function UpdateBanner({ u, onError }: { u: UpdateView; onError: (m: string) => void }) {
  if (!["available", "downloading", "installing"].includes(u.status) || !u.version) return null;
  const pct = u.progress != null ? Math.round(u.progress * 100) : null;
  return (
    <div className="update-bar" role="status">
      <Icon name="refresh" />
      {u.status === "available" && <span>Version {u.version} is available</span>}
      {u.status === "downloading" && <span>Downloading {u.version}{pct != null ? ` · ${pct}%` : ""}</span>}
      {u.status === "installing" && <span>Installing {u.version}, the app will restart</span>}
      {u.status === "available" && (
        <button className="btn secondary small" onClick={() => api.updateInstall().catch((e) => onError(String(e)))}>
          Install and restart
        </button>
      )}
    </div>
  );
}

function SettingsPage({
  s,
  onSave,
  update,
  onUpdate,
  onError,
}: {
  s: Settings;
  onSave: (s: Settings) => void;
  update: UpdateView | null;
  onUpdate: (u: UpdateView) => void;
  onError: (m: string) => void;
}) {
  const set = (patch: Partial<Settings>) => onSave({ ...s, ...patch });
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Settings</h1>
          <p className="page-sub">Changes save right away</p>
        </div>
      </header>
      <section className="panel form">
        <h2>Free games</h2>
        <label className="row">
          <span>Check every</span>
          <select value={s.checkIntervalHours} onChange={(e) => set({ checkIntervalHours: Number(e.target.value) })}>
            {[1, 3, 6, 12, 24].map((h) => (
              <option key={h} value={h}>
                {h} hour{h > 1 ? "s" : ""}, give or take 20 min
              </option>
            ))}
          </select>
        </label>
        <Toggle label="Epic Games Store" v={s.epic} on={(epic) => set({ epic })} />
        <Toggle label="Steam 100% off games" v={s.steam} on={(steam) => set({ steam })} />
        <Toggle label="GOG giveaways" v={s.gog} on={(gog) => set({ gog })} />
        <Toggle label="Prime Gaming monthly reminder" v={s.prime} on={(prime) => set({ prime })} />
      </section>
      <section className="panel form">
        <h2>Steam auto-add</h2>
        <p className="form-note">
          Adds new 100%-off Steam games to your account in a hidden Steam window. Use Add to account once first so
          you're signed in. It adds one game at a time with random gaps and stops at the daily limit.
        </p>
        <Toggle label="Auto-add free Steam games" v={s.steamAutoAdd} on={(steamAutoAdd) => set({ steamAutoAdd })} />
        <label className="row">
          <span>Daily limit</span>
          <select value={s.steamDailyCap} onChange={(e) => set({ steamDailyCap: Number(e.target.value) })}>
            {[3, 5, 10, 20].map((n) => (
              <option key={n} value={n}>
                {n} games a day
              </option>
            ))}
          </select>
        </label>
      </section>
      <section className="panel form">
        <h2>App</h2>
        <Toggle label="Desktop notifications for new free games" v={s.notify} on={(notify) => set({ notify })} />
        <Toggle label="Close button hides to tray" v={s.closeToTray} on={(closeToTray) => set({ closeToTray })} />
        <Toggle label="Start with Windows, hidden in the tray" v={s.launchAtStartup} on={(launchAtStartup) => set({ launchAtStartup })} />
      </section>
      <section className="panel form">
        <h2>Updates</h2>
        <p className="form-note">
          Updates come from GitHub Releases and are checked for a valid signature before they install. Automatic
          installs only happen while the window is in the tray and card idling is off.
        </p>
        <Toggle label="Install updates automatically" v={s.autoUpdate} on={(autoUpdate) => set({ autoUpdate })} />
        <div className="row static">
          <span>
            Version <span className="mono">{update?.current ?? "unknown"}</span>
            <span className="update-note">{updateNote(update)}</span>
          </span>
          <button
            className="btn secondary small"
            disabled={update?.status === "checking" || update?.status === "downloading"}
            onClick={() => api.updateCheck().then(onUpdate).catch((e) => onError(String(e)))}
          >
            Check for updates
          </button>
        </div>
      </section>
    </div>
  );
}

function Toggle({ label, v, on }: { label: string; v: boolean; on: (v: boolean) => void }) {
  return (
    <label className="row">
      <span>{label}</span>
      <input type="checkbox" className="switch" checked={v} onChange={(e) => on(e.target.checked)} />
    </label>
  );
}

function updateNote(u: UpdateView | null): string {
  if (!u) return "";
  switch (u.status) {
    case "checking":
      return " · checking";
    case "latest":
      return " · up to date";
    case "available":
      return ` · ${u.version} available`;
    case "downloading":
      return ` · downloading ${u.version}`;
    case "installing":
      return " · installing";
    case "error":
      return " · couldn't check for updates";
    default:
      return "";
  }
}
