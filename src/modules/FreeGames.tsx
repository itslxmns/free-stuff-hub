import { useState } from "react";
import { Icon } from "../lib/icons";
import { api, relTime, timeLeft, type GameView, type Snapshot, type Store } from "../lib/api";

const STORE_LABEL: Record<Store, string> = { epic: "Epic Games", gog: "GOG", prime: "Prime Gaming", steam: "Steam" };
const DAY = 86_400_000;

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
  const toClaim = snap.games.filter((g) => g.status !== "upcoming" && !g.claimed).length;
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
          <p className="page-sub">
            Epic, Steam, GOG and Prime Gaming · checked {relTime(snap.lastCheck)}, next {relTime(snap.nextCheck)}
          </p>
        </div>
        <button className="btn primary" onClick={checkNow} disabled={busy}>
          <Icon name="refresh" />
          {busy ? "Checking" : "Check now"}
        </button>
      </header>

      <div className="stats">
        <Stat label="Free to claim" value={toClaim} lead={toClaim > 0} />
        <Stat label="Coming soon" value={upcoming} />
        <Stat label="Claimed so far" value={snap.claimedTotal} />
      </div>

      <p className="assurance">
        <Icon name="shield" />
        Reads public store pages only. You sign in on each store's own page, and Free Stuff Hub never sees your
        passwords or tokens. On Steam, Add to account presses Steam's own button in your signed-in page.
      </p>

      <div className="toolbar">
        <div className="segmented" role="tablist" aria-label="Store">
          {(["all", "epic", "steam", "gog", "prime"] as const).map((f) => (
            <button
              key={f}
              role="tab"
              aria-selected={filter === f}
              className={filter === f ? "on" : ""}
              onClick={() => setFilter(f)}
            >
              {f === "all" ? "All" : STORE_LABEL[f]}
            </button>
          ))}
        </div>
        <label className="inline-toggle">
          <input type="checkbox" className="switch" checked={hideClaimed} onChange={(e) => setHideClaimed(e.target.checked)} />
          Hide claimed
        </label>
      </div>

      {games.length === 0 ? (
        <p className="empty">
          {snap.lastCheck ? "Nothing free right now. You'll get a notification when something new shows up" : "Running the first check"}
        </p>
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

function Stat({ label, value, lead }: { label: string; value: number; lead?: boolean }) {
  return (
    <div className="stat">
      <div className="stat-label">{label}</div>
      <div className={`stat-value ${lead ? "lead" : ""}`}>{value}</div>
    </div>
  );
}

const fmtDay = (iso: string) =>
  new Date(iso).toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" });

/** The signature: a mono deal tag, like a price sticker with a timer. */
function dealTag(g: GameView): { text: string; urgent: boolean } {
  if (g.status === "upcoming") return { text: g.startsAt ? `Free from ${fmtDay(g.startsAt)}` : "Free soon", urgent: false };
  if (g.status === "reminder") return { text: `Monthly · ${new Date().toLocaleDateString("en-GB", { month: "long" })}`, urgent: false };
  if (!g.endsAt) return { text: "Free now · no end date listed", urgent: false };
  const ms = new Date(g.endsAt).getTime() - Date.now();
  return { text: `Free until ${fmtDay(g.endsAt)} · ${timeLeft(g.endsAt)}`, urgent: ms > 0 && ms < DAY };
}

function GameCard({ g, onSnap, onError }: { g: GameView; onSnap: (s: Snapshot) => void; onError: (m: string) => void }) {
  const [artFailed, setArtFailed] = useState(false);
  const run = (p: Promise<unknown>) => p.catch((e) => onError(String(e)));
  const tag = dealTag(g);
  const showArt = g.image && !artFailed;

  return (
    <article className={`card ${g.claimed ? "claimed" : ""}`}>
      <div className="cover">
        {showArt ? (
          <img src={g.image!} alt="" loading="lazy" onError={() => setArtFailed(true)} />
        ) : g.status === "reminder" ? (
          <div className="cover-month">
            <span>{new Date().toLocaleDateString("en-GB", { month: "long" })}</span>
            <small>{STORE_LABEL[g.store]}</small>
          </div>
        ) : (
          <div className="cover-blank">{STORE_LABEL[g.store]}</div>
        )}
      </div>
      <div className="card-body">
        <div className="card-store">
          {STORE_LABEL[g.store]}
          {g.isNew && !g.claimed && <span className="new">New</span>}
        </div>
        <h3 title={g.title}>{g.title}</h3>
        <div className={`deal ${tag.urgent ? "urgent" : ""} ${g.status === "upcoming" ? "later" : ""}`}>
          <Icon name="tag" size={14} />
          {tag.text}
        </div>
        {g.note && <p className="card-note">{g.note}</p>}
        <div className="card-actions">
          {g.store === "steam" && !g.claimed && (
            <button
              className="btn secondary"
              title="Opens the Steam page and presses Steam's own Add to Account button"
              onClick={() => run(api.steamAddToAccount(g.id))}
            >
              Add to account
            </button>
          )}
          {g.store !== "steam" && g.status !== "upcoming" && !g.claimed && (
            <button className="btn secondary" onClick={() => run(api.openStoreWindow(g.url, `Claim: ${g.title}`))}>
              Claim
            </button>
          )}
          {g.status !== "upcoming" && (
            <button
              className={`btn ghost ${g.claimed ? "is-claimed" : ""}`}
              onClick={() => run(api.setClaimed(g.id, !g.claimed).then(onSnap))}
            >
              {g.claimed && <Icon name="check" />}
              {g.claimed ? "Claimed" : "Mark claimed"}
            </button>
          )}
          <button
            className="btn icon ghost push"
            title="Open in your browser"
            aria-label={`Open ${g.title} in your browser`}
            onClick={() => run(api.openExternal(g.url))}
          >
            <Icon name="external" />
          </button>
        </div>
      </div>
    </article>
  );
}
