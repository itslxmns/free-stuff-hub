import { useState } from "react";
import { Icon } from "../lib/icons";
import { api, relTime, timeLeft, type GameView, type Snapshot, type Store } from "../lib/api";

const STORE_LABEL: Record<Store, string> = { epic: "Epic Games", gog: "GOG", prime: "Prime Gaming", steam: "Steam" };

interface Props {
  snap: Snapshot;
  onSnap: (s: Snapshot) => void;
  onError: (msg: string) => void;
}

export function FreeGames({ snap, onSnap, onError }: Props) {
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState<"all" | Store>("all");
  const [hideClaimed, setHideClaimed] = useState(false);

  const games = snap.games.filter(
    (g) => (filter === "all" || g.store === filter) && !(hideClaimed && g.claimed),
  );
  const activeUnclaimed = snap.games.filter((g) => g.status !== "upcoming" && !g.claimed).length;
  const upcoming = snap.games.filter((g) => g.status === "upcoming").length;

  async function checkNow() {
    setBusy(true);
    try {
      onSnap(await api.checkNow());
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Free games</h1>
          <p className="muted">
            Epic, Steam, GOG and Prime Gaming. Last check {relTime(snap.lastCheck)} · next {relTime(snap.nextCheck)}
          </p>
        </div>
        <button className="btn primary" onClick={checkNow} disabled={busy}>
          {busy ? "Checking..." : "Check now"}
        </button>
      </header>

      <div className="stats">
        <Stat label="Free to claim" value={activeUnclaimed} accent />
        <Stat label="Coming soon" value={upcoming} />
        <Stat label="Claimed so far" value={snap.claimedTotal} />
      </div>

      <div className="notice safe">
        <strong>Low risk.</strong> This module reads public store listings. Claiming happens on the store's
        official page, where you sign in yourself. Free Stuff Hub never sees your password or tokens. Steam's{" "}
        <b>Add to account</b> clicks Steam's own button in your signed-in Steam page, one game at a time.
      </div>

      <div className="toolbar">
        {(["all", "epic", "steam", "gog", "prime"] as const).map((f) => (
          <button key={f} className={`chip ${filter === f ? "on" : ""}`} onClick={() => setFilter(f)}>
            {f === "all" ? "All" : STORE_LABEL[f]}
          </button>
        ))}
        <label className="check">
          <input type="checkbox" checked={hideClaimed} onChange={(e) => setHideClaimed(e.target.checked)} />
          Hide claimed
        </label>
      </div>

      {games.length === 0 ? (
        <div className="empty">
          {snap.lastCheck ? "Nothing here right now. New offers trigger a notification." : "Running first check..."}
        </div>
      ) : (
        <div className="grid">
          {games.map((g) => (
            <GameCard key={g.id} g={g} onSnap={onSnap} onError={onError} />
          ))}
        </div>
      )}
    </div>
  );
}

function Stat({ label, value, accent }: { label: string; value: number; accent?: boolean }) {
  return (
    <div className={`stat ${accent ? "accent" : ""}`}>
      <div className="stat-value">{value}</div>
      <div className="stat-label">{label}</div>
    </div>
  );
}

function GameCard({ g, onSnap, onError }: { g: GameView; onSnap: (s: Snapshot) => void; onError: (m: string) => void }) {
  const left = g.status === "active" ? timeLeft(g.endsAt) : null;
  const startsIn = g.status === "upcoming" ? relTime(g.startsAt) : null;
  const run = (p: Promise<unknown>) => p.catch((e) => onError(String(e)));

  return (
    <article className={`card ${g.claimed ? "claimed" : ""}`}>
      <div className={`cover store-${g.store}`}>
        {g.image ? <img src={g.image} alt="" loading="lazy" /> : <span>{STORE_LABEL[g.store]}</span>}
        <span className={`badge ${g.status}`}>
          {g.status === "active" ? "FREE NOW" : g.status === "upcoming" ? "UPCOMING" : "MONTHLY"}
        </span>
        {g.isNew && <span className="badge new">NEW</span>}
      </div>
      <div className="card-body">
        <div className="card-store">{STORE_LABEL[g.store]}</div>
        <h3 title={g.title}>{g.title}</h3>
        <div className="card-meta">
          {left && <span className="meta-line"><Icon name="clock" size={13} /> {left} left</span>}
          {startsIn && <span className="meta-line"><Icon name="calendar" size={13} /> free {startsIn}</span>}
          {g.note && <span className="note">{g.note}</span>}
        </div>
        <div className="card-actions">
          {g.store === "steam" && !g.claimed && (
            <button
              className="btn primary"
              title="Opens the Steam page and clicks Steam's own 'Add to Account' button for you"
              onClick={() => run(api.steamAddToAccount(g.id))}
            >
              Add to account
            </button>
          )}
          {g.store !== "steam" && g.status !== "upcoming" && (
            <button className="btn primary" onClick={() => run(api.openStoreWindow(g.url, `Claim: ${g.title}`))}>
              Claim
            </button>
          )}
          <button className="btn ghost" title="Open in your default browser" onClick={() => run(api.openExternal(g.url))}>
            <Icon name="external" size={13} /> Browser
          </button>
          {g.status !== "upcoming" && (
            <button
              className={`btn ${g.claimed ? "ghost" : "outline"}`}
              onClick={() => run(api.setClaimed(g.id, !g.claimed).then(onSnap))}
            >
              {g.claimed ? <><Icon name="check" size={13} /> Claimed</> : "Mark claimed"}
            </button>
          )}
        </div>
      </div>
    </article>
  );
}
