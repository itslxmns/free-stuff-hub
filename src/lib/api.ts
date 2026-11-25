import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { mockApi } from "./mock";

export type Store = "epic" | "gog" | "prime" | "steam";
export type OfferStatus = "active" | "upcoming" | "reminder";

export interface GameView {
  id: string;
  store: Store;
  title: string;
  url: string;
  image: string | null;
  startsAt: string | null;
  endsAt: string | null;
  status: OfferStatus;
  note: string | null;
  claimed: boolean;
  isNew: boolean;
}

export interface Settings {
  checkIntervalHours: number;
  notify: boolean;
  closeToTray: boolean;
  launchAtStartup: boolean;
  epic: boolean;
  gog: boolean;
  prime: boolean;
  steam: boolean;
  steamAutoAdd: boolean;
  steamDailyCap: number;
  idleMaxGames: number;
  idleMulti: boolean;
  acknowledgedRisk: boolean;
}

export interface LogEntry {
  at: string;
  module: string;
  level: "info" | "error" | "reward" | string;
  message: string;
}

export interface Snapshot {
  games: GameView[];
  settings: Settings;
  lastCheck: string | null;
  nextCheck: string | null;
  log: LogEntry[];
  claimedTotal: number;
}

export interface IdleGame {
  appid: number;
  title: string;
  drops: number;
  hours: number;
  idling: boolean;
  refused: boolean;
}

export interface IdleView {
  running: boolean;
  status: string;
  needsLogin: boolean;
  steamReady: boolean;
  dllFound: boolean;
  games: IdleGame[];
  totalDrops: number;
  lastRefresh: string | null;
  nextSwitch: string | null;
}

const tauriApi = {
  snapshot: () => invoke<Snapshot>("get_snapshot"),
  checkNow: () => invoke<Snapshot>("check_now"),
  setClaimed: (id: string, claimed: boolean) => invoke<Snapshot>("set_claimed", { id, claimed }),
  updateSettings: (settings: Settings) => invoke<Snapshot>("update_settings", { settings }),
  openStoreWindow: (url: string, title?: string) => invoke<void>("open_store_window", { url, title }),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  steamAddToAccount: (id: string) => invoke<void>("steam_add_to_account", { id }),
  idleState: () => invoke<IdleView>("idle_state"),
  idleStart: () => invoke<IdleView>("idle_start"),
  idleStop: () => invoke<IdleView>("idle_stop"),
  idleSignIn: () => invoke<void>("idle_sign_in"),
  onIdle: (cb: (v: IdleView) => void): Promise<UnlistenFn> => listen<IdleView>("idle", (e) => cb(e.payload)),
  onSnapshot: (cb: (s: Snapshot) => void): Promise<UnlistenFn> =>
    listen<Snapshot>("snapshot", (e) => cb(e.payload)),
};

/** True inside the Tauri window; false in a plain browser (demo mode with sample data). */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: typeof tauriApi = isTauri ? tauriApi : (mockApi as unknown as typeof tauriApi);

export function timeLeft(iso: string | null): string | null {
  if (!iso) return null;
  const ms = new Date(iso).getTime() - Date.now();
  if (ms <= 0) return "ended";
  const h = Math.floor(ms / 3_600_000);
  const d = Math.floor(h / 24);
  if (d >= 1) return `${d}d ${h % 24}h`;
  const m = Math.floor((ms % 3_600_000) / 60_000);
  return `${h}h ${m}m`;
}

export function relTime(iso: string | null): string {
  if (!iso) return "never";
  const diff = new Date(iso).getTime() - Date.now();
  const abs = Math.abs(diff);
  const mins = Math.round(abs / 60_000);
  const txt = mins < 60 ? `${mins} min` : mins < 60 * 48 ? `${Math.round(mins / 60)} h` : `${Math.round(mins / 1440)} d`;
  return diff < 0 ? `${txt} ago` : `in ${txt}`;
}
