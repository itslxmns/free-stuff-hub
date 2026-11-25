import { useEffect, useState } from "react";
import { Icon } from "../lib/icons";
import { api, relTime, type IdleView, type Settings } from "../lib/api";

interface Props {
  settings: Settings;
  onSaveSettings: (s: Settings) => void;
  onError: (msg: string) => void;
  onIdle: (v: IdleView) => void;
  idle: IdleView | null;
}

const hrs = (h: number) => `${h.toFixed(1)} h`;

export function SteamIdle({ settings, onSaveSettings, onError, idle, onIdle }: Props) {
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.idleState().then(onIdle).catch((e) => onError(String(e)));
  }, []);

  if (!idle) return <div className="page"><p className="empty">Loading</p></div>;

  const run = async (p: Promise<IdleView | void>) => {
    setBusy(true);
    try {
      const v = await p;
      if (v) onIdle(v);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const idlingNow = idle.games.filter((g) => g.idling).length;
  const set = (patch: Partial<Settings>) => onSaveSettings({ ...settings, ...patch });

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Steam card idling</h1>
          <p className="page-sub">
            {idle.lastRefresh ? `Drops checked ${relTime(idle.lastRefresh)}` : "Drops not checked yet"}
            {idle.nextSwitch ? ` · next switch ${relTime(idle.nextSwitch)}` : ""}
          </p>
        </div>
        {idle.running ? (
          <button className="btn secondary" disabled={busy} onClick={() => run(api.idleStop())}>
            Stop idling
          </button>
        ) : (
          <button className="btn primary" disabled={busy || !idle.steamReady} onClick={() => run(api.idleStart())}>
            Start idling
          </button>
        )}
      </header>

      <div className={`status-line ${idle.running ? "live" : ""}`}>
        <span className="status-dot" />
        {!idle.steamReady && !idle.running ? "Open Steam and sign in to start idling" : idle.status}
      </div>

      <div className="stats">
        <div className="stat">
          <div className="stat-label">Card drops left</div>
          <div className={`stat-value ${idle.totalDrops > 0 ? "lead" : ""}`}>{idle.totalDrops}</div>
        </div>
        <div className="stat">
          <div className="stat-label">Games with drops</div>
          <div className="stat-value">{idle.games.length}</div>
        </div>
        <div className="stat">
          <div className="stat-label">Idling now</div>
          <div className="stat-value">{idlingNow}</div>
        </div>
      </div>

      <p className="assurance">
        <Icon name="shield" />
        Uses the Steam client you're signed in to and Steam's own API, so no game actually runs. Friends see you as in
        game, and the hours count on your profile. Low ToS risk: Valve has tolerated card idling for years.
      </p>

      {(idle.needsLogin || !idle.lastRefresh) && (
        <section className="panel callout">
          <div>
            <h2>Sign in to Steam Community once</h2>
            <p>
              Card drops are listed on your badges page. Sign in on Steam's own page in the window that opens. The app
              only reads the drop counts.
            </p>
          </div>
          <button className="btn secondary" onClick={() => run(api.idleSignIn())}>
            Open Steam Community
          </button>
        </section>
      )}

      {idle.games.length > 0 && (
        <section className="panel table-panel">
          <div className="t-row t-head">
            <span>Game</span>
            <span className="num">Playtime</span>
            <span className="num">Drops left</span>
            <span>State</span>
          </div>
          {idle.games.map((g) => (
            <div key={g.appid} className={`t-row ${g.idling ? "on" : ""}`}>
              <span className="t-title">
                {g.title || `App ${g.appid}`}
                <small>{g.appid}</small>
              </span>
              <span className="num mono">{hrs(g.hours)}</span>
              <span className="num mono">{g.drops}</span>
              <span
                className={`t-state ${g.idling ? "on" : ""} ${g.refused ? "refused" : ""}`}
                title={g.refused ? "Steam wouldn't start this game. It may be Family Shared, a free weekend or not fully owned" : undefined}
              >
                {g.refused ? "Steam refused" : g.idling ? "Idling" : g.hours < 2 ? "Queued, under 2 h" : "Queued"}
              </span>
            </div>
          ))}
        </section>
      )}

      <section className="panel form">
        <h2>How it idles</h2>
        <p className="form-note">
          Many games only drop cards after 2 hours of playtime. Games under 2 h idle together first, then the rest go
          one at a time. Drops are re-checked every 15 to 30 minutes at random, and idling pauses while you play.
        </p>
        <label className="row">
          <span>Idle games under 2 h together</span>
          <input type="checkbox" className="switch" checked={settings.idleMulti} onChange={(e) => set({ idleMulti: e.target.checked })} />
        </label>
        <label className="row">
          <span>Games at once</span>
          <select value={settings.idleMaxGames} onChange={(e) => set({ idleMaxGames: Number(e.target.value) })}>
            {[1, 3, 5, 10].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
      </section>
    </div>
  );
}
