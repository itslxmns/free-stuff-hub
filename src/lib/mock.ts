// Demo data used only when the UI runs in a plain browser (no Tauri backend),
// e.g. `npm run dev` opened at http://localhost:1420 for UI work.
import type { IdleView, Settings, Snapshot, UpdateView } from "./api";

const day = 86_400_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();

let state: Snapshot = {
  settings: { checkIntervalHours: 6, notify: true, closeToTray: true, launchAtStartup: false, epic: true, gog: true, prime: true, steam: true, steamAutoAdd: false, steamDailyCap: 10, idleMaxGames: 5, idleMulti: true, autoUpdate: true, acknowledgedRisk: true },
  lastCheck: iso(-12 * 60_000),
  nextCheck: iso(6 * 3_600_000),
  claimedTotal: 1,
  log: [
    { at: iso(-12 * 60_000), module: "free-games", level: "reward", message: "New free game: Demo Game One (Epic)" },
    { at: iso(-12 * 60_000), module: "free-games", level: "info", message: "Check (scheduled): 4 offers, 1 new" },
  ],
  games: [
    { id: "epic:mark-of-the-deep", store: "epic", title: "Mark of the Deep", url: "https://store.epicgames.com/en-US/p/mark-of-the-deep-f7dc08", image: "https://cdn1.epicgames.com/spt-assets/83a240506a884c10ad8fb99694e57a0d/mark-of-the-deep-w1ygr.png?w=720&resize=1&quality=high", startsAt: iso(-5 * day), endsAt: iso(18 * 3_600_000), status: "active", note: null, claimed: false, isNew: true },
    { id: "steam:App_demo", store: "steam", title: "Sample Steam giveaway", url: "https://store.steampowered.com/", image: null, startsAt: null, endsAt: null, status: "active", note: "100% off on Steam. Free to keep if you add it before the promo ends.", claimed: false, isNew: true },
    { id: "prime:demo", store: "prime", title: "Prime Gaming games for this month", url: "https://gaming.amazon.com/home", image: null, startsAt: null, endsAt: null, status: "reminder", note: "Needs an Amazon Prime membership. Open the claims page, sign in, and claim this month's games.", claimed: false, isNew: false },
    { id: "gog:demo", store: "gog", title: "Sample GOG giveaway", url: "https://www.gog.com/en/", image: null, startsAt: null, endsAt: iso(3 * day), status: "active", note: "100% off on GOG. Claim it while the deal lasts.", claimed: true, isNew: false },
    { id: "epic:bread-and-fred", store: "epic", title: "Bread and Fred", url: "https://store.epicgames.com/en-US/p/bread-and-fred-1db362", image: "https://cdn1.epicgames.com/spt-assets/545b4dd8bb364c12b719268198a93ef3/bread-and-fred-3jnwn.jpg?w=720&resize=1&quality=high", startsAt: iso(19 * 3_600_000), endsAt: iso(7 * day), status: "upcoming", note: null, claimed: false, isNew: false },
    { id: "epic:firestone", store: "epic", title: "Firestone: Idle Clicker Online RPG", url: "https://store.epicgames.com/en-US/p/firestone-online-idle-rpg-bfd04b", image: "https://cdn1.epicgames.com/spt-assets/69576a28f7154fe9a0dce97508da5163/firestone-online-idle-rpg-1inmf.jpg?w=720&resize=1&quality=high", startsAt: iso(19 * 3_600_000), endsAt: iso(7 * day), status: "upcoming", note: null, claimed: false, isNew: false },
  ],
};

let idle: IdleView = {
  running: true,
  status: "Building playtime on 3 games at once",
  needsLogin: false,
  steamReady: true,
  dllFound: true,
  totalDrops: 14,
  lastRefresh: iso(-4 * 60_000),
  nextSwitch: iso(21 * 60_000),
  games: [
    { appid: 413150, title: "Stardew Valley", drops: 4, hours: 1.4, idling: true, refused: false },
    { appid: 892970, title: "Valheim", drops: 3, hours: 0.6, idling: true, refused: false },
    { appid: 3241660, title: "R.E.P.O.", drops: 3, hours: 0.2, idling: true, refused: false },
    { appid: 108600, title: "Project Zomboid", drops: 4, hours: 37.5, idling: false, refused: false },
  ],
};

let update: UpdateView = { current: "0.3.0", status: "available", version: "0.3.1", notes: null, progress: null, error: null };

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
  steamAddToAccount: async () => {},
  idleState: async () => idle,
  idleStart: async () => (idle = { ...idle, running: true, status: "Starting" }),
  idleStop: async () => (idle = { ...idle, running: false, status: "Stopped", nextSwitch: null, games: idle.games.map((g) => ({ ...g, idling: false, refused: false })) }),
  idleSignIn: async () => {},
  updateState: async () => update,
  updateCheck: async () => update,
  updateInstall: async () => {
    update = { ...update, status: "downloading", progress: 0.4 };
  },
  onUpdate: async () => () => {},
  onIdle: async () => () => {},
  onSnapshot: async () => () => {},
};
