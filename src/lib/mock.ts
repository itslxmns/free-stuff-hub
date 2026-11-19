// Demo data used only when the UI runs in a plain browser (no Tauri backend),
// e.g. `npm run dev` opened at http://localhost:1420 for UI work.
import type { Settings, Snapshot } from "./api";

const day = 86_400_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();

let state: Snapshot = {
  settings: { checkIntervalHours: 6, notify: true, closeToTray: true, epic: true, gog: true, prime: true, acknowledgedRisk: false },
  lastCheck: iso(-12 * 60_000),
  nextCheck: iso(6 * 3_600_000),
  claimedTotal: 0,
  log: [
    { at: iso(-12 * 60_000), module: "free-games", level: "reward", message: "New free game: Demo Game One (Epic)" },
    { at: iso(-12 * 60_000), module: "free-games", level: "info", message: "Check (scheduled): 4 offers, 1 new" },
  ],
  games: [
    { id: "epic:demo1", store: "epic", title: "Demo Game One", url: "https://store.epicgames.com/en-US/free-games", image: null, startsAt: iso(-day), endsAt: iso(6 * day), status: "active", note: null, claimed: false, isNew: true },
    { id: "prime:demo", store: "prime", title: "Prime Gaming games for this month", url: "https://gaming.amazon.com/home", image: null, startsAt: null, endsAt: null, status: "reminder", note: "Needs an Amazon Prime membership. Open the claims page, sign in, and claim this month's games.", claimed: false, isNew: false },
    { id: "gog:demo", store: "gog", title: "Demo GOG Giveaway", url: "https://www.gog.com/en/", image: null, startsAt: null, endsAt: null, status: "active", note: "100% off on GOG. Claim it while the deal lasts.", claimed: false, isNew: false },
    { id: "epic:demo2", store: "epic", title: "Demo Game Two", url: "https://store.epicgames.com/en-US/free-games", image: null, startsAt: iso(7 * day), endsAt: iso(14 * day), status: "upcoming", note: null, claimed: false, isNew: false },
  ],
};

export const mockApi = {
  snapshot: async () => state,
  checkNow: async () => (state = { ...state, lastCheck: new Date().toISOString() }),
  setClaimed: async (id: string, claimed: boolean) => {
    const games = state.games.map((g) => (g.id === id ? { ...g, claimed } : g));
    return (state = { ...state, games, claimedTotal: games.filter((g) => g.claimed).length });
  },
  updateSettings: async (settings: Settings) => (state = { ...state, settings }),
  openStoreWindow: async (url: string) => void window.open(url, "_blank"),
  openExternal: async (url: string) => void window.open(url, "_blank"),
  onSnapshot: async () => () => {},
};
