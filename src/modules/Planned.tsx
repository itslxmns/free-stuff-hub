import { Icon, type IconName } from "../lib/icons";

export type Risk = "low" | "medium" | "high";

export interface PlannedModule {
  id: string;
  name: string;
  icon: IconName;
  risk: Risk;
  summary: string;
  riskText: string;
  safeguards: string[];
}

export const PLANNED: PlannedModule[] = [
  {
    id: "discord",
    name: "Discord quests",
    icon: "quest",
    risk: "high",
    summary:
      "Detect new Discord quests, run a stand-in for the required game process in the background, and log the rewards.",
    riskText:
      "Spoofing game activity breaks Discord's Terms of Service. Discord has flagged and revoked quest rewards for accounts that do this, and accounts can be restricted. Only use it on an account you're willing to lose.",
    safeguards: [
      "One quest at a time, never in parallel",
      "Random start delays and play durations with jitter",
      "Sign in through Discord's official login page only (no token pasting)",
      "Opt-in per quest, with a daily cap and a kill switch in the tray",
    ],
  },
  {
    id: "steam",
    name: "Steam card idling",
    icon: "cards",
    risk: "low",
    summary: "Mark games as running so they drop trading cards, like Idle Master or ArchiSteamFarm.",
    riskText:
      "Widely used and generally tolerated, but it's still automation on your Steam account. Playtime shows up on your profile.",
    safeguards: [
      "Uses the local Steam client you're already signed in to",
      "Caps how many games idle at once, rotating at random intervals",
      "Stops when you launch a real game",
    ],
  },
  {
    id: "drops",
    name: "Twitch & Kick drops",
    icon: "tv",
    risk: "medium",
    summary: "Watch eligible streams in a hidden window to progress drop campaigns, like TwitchDropsMiner.",
    riskText:
      "Automated viewing is a grey area under Twitch's and Kick's terms. Drops can be withheld, and accounts can be limited.",
    safeguards: [
      "Sign in on the official Twitch/Kick page inside the app",
      "One stream per campaign, at the lowest quality",
      "Switches channels at random intervals, with a daily time limit",
    ],
  },
];

export function PlannedPage({ m }: { m: PlannedModule }) {
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1 className="title-icon">
            <Icon name={m.icon} size={22} /> {m.name}
          </h1>
          <p className="muted">{m.summary}</p>
        </div>
        <span className={`risk risk-${m.risk}`}>{m.risk.toUpperCase()} RISK</span>
      </header>
      <div className={`notice ${m.risk === "high" ? "danger" : m.risk === "medium" ? "warn" : "safe"}`}>
        <strong>Terms of Service warning.</strong> {m.riskText}
      </div>
      <section className="panel">
        <h2>Planned safeguards</h2>
        <ul className="list">
          {m.safeguards.map((s) => (
            <li key={s}>{s}</li>
          ))}
        </ul>
      </section>
      <div className="empty">This module isn't built yet. Free games is the first module.</div>
    </div>
  );
}
