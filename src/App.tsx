import { useEffect, useState } from "react";
import "./App.css";
import { api, isTauri, type Settings, type Snapshot } from "./lib/api";
import { Icon, type IconName } from "./lib/icons";
import { FreeGames } from "./modules/FreeGames";
import { PLANNED, PlannedPage } from "./modules/Planned";

type Page = "free-games" | "log" | "settings" | (typeof PLANNED)[number]["id"];

export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [page, setPage] = useState<Page>("free-games");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.snapshot().then(setSnap).catch((e) => setError(String(e)));
    const un = api.onSnapshot(setSnap);
    return () => {
      un.then((f) => f());
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
          <span className="logo"><Icon name="gift" size={22} /></span>
          <div>
            <div className="brand-name">Free Stuff Hub</div>
            <div className="brand-sub">free rewards, one place</div>
          </div>
        </div>
        <nav>
          <div className="nav-label">Modules</div>
          <NavItem icon="gamepad" label="Free games" active={page === "free-games"} onClick={() => setPage("free-games")} count={freeCount} />
          {PLANNED.map((m) => (
            <NavItem key={m.id} icon={m.icon} label={m.name} active={page === m.id} onClick={() => setPage(m.id)} tag="soon" risk={m.risk} />
          ))}
          <div className="nav-label">App</div>
          <NavItem icon="log" label="Activity log" active={page === "log"} onClick={() => setPage("log")} />
          <NavItem icon="settings" label="Settings" active={page === "settings"} onClick={() => setPage("settings")} />
        </nav>
        <div className="sidebar-foot muted">Closing the window keeps the app running in the tray.</div>
      </aside>

      <main className="content">
        {!isTauri && <div className="demo-bar">Browser demo mode: sample data, no backend. Run <code>npm run tauri dev</code> for the real app.</div>}
        {page === "free-games" && <FreeGames snap={snap} onSnap={setSnap} onError={setError} />}
        {planned && <PlannedPage m={planned} />}
        {page === "log" && <ActivityLog snap={snap} />}
        {page === "settings" && <SettingsPage s={snap.settings} onSave={saveSettings} />}
      </main>

      {!snap.settings.acknowledgedRisk && (
        <div className="modal-backdrop">
          <div className="modal">
            <h2>Before you start</h2>
            <p>
              Free Stuff Hub automates reward farming. Some planned modules (<b>Discord quests</b>, <b>Twitch/Kick drops</b>)
              break those platforms' Terms of Service and can get rewards revoked or accounts flagged. They'll ship with
              safeguards (random timing, one task at a time), but the risk is yours.
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
        <div className="toast" onClick={() => setError(null)}>
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
  risk?: string;
}) {
  return (
    <button className={`nav-item ${props.active ? "active" : ""}`} onClick={props.onClick}>
      <span className="nav-icon"><Icon name={props.icon} /></span>
      <span className="nav-text">{props.label}</span>
      {!!props.count && <span className="nav-count">{props.count}</span>}
      {props.risk && <span className={`dot risk-${props.risk}`} title={`${props.risk} ToS risk`} />}
      {props.tag && <span className="nav-tag">{props.tag}</span>}
    </button>
  );
}

function ActivityLog({ snap }: { snap: Snapshot }) {
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Activity log</h1>
          <p className="muted">Checks, new offers and claimed rewards (newest first).</p>
        </div>
      </header>
      <div className="panel log">
        {snap.log.length === 0 && <div className="muted">No activity yet.</div>}
        {snap.log.map((l, i) => (
          <div key={i} className={`log-row lvl-${l.level}`}>
            <span className="log-time">{new Date(l.at).toLocaleString()}</span>
            <span className="log-mod">{l.module}</span>
            <span className="log-msg">{l.message}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function SettingsPage({ s, onSave }: { s: Settings; onSave: (s: Settings) => void }) {
  const set = (patch: Partial<Settings>) => onSave({ ...s, ...patch });
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Settings</h1>
          <p className="muted">Changes save right away.</p>
        </div>
      </header>
      <section className="panel form">
        <h2>Free games</h2>
        <label className="row">
          <span>Check every</span>
          <select value={s.checkIntervalHours} onChange={(e) => set({ checkIntervalHours: Number(e.target.value) })}>
            {[1, 3, 6, 12, 24].map((h) => (
              <option key={h} value={h}>
                {h} hour{h > 1 ? "s" : ""} (+/- 20 min random)
              </option>
            ))}
          </select>
        </label>
        <Toggle label="Epic Games Store" v={s.epic} on={(epic) => set({ epic })} />
        <Toggle label="GOG giveaways" v={s.gog} on={(gog) => set({ gog })} />
        <Toggle label="Prime Gaming monthly reminder" v={s.prime} on={(prime) => set({ prime })} />
        <h2>App</h2>
        <Toggle label="Desktop notifications for new free games" v={s.notify} on={(notify) => set({ notify })} />
        <Toggle label="Close button hides to tray" v={s.closeToTray} on={(closeToTray) => set({ closeToTray })} />
        <Toggle label="Start with Windows (hidden in tray)" v={s.launchAtStartup} on={(launchAtStartup) => set({ launchAtStartup })} />
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
