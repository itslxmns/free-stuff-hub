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

const RISK_LABEL: Record<Risk, string> = { low: "Low ToS risk", medium: "Medium ToS risk", high: "High ToS risk" };

export function PlannedPage({ m }: { m: PlannedModule }) {
  return (
    <div className="page narrow">
      <header className="page-head">
        <div>
          <h1>{m.name}</h1>
          <p className="page-sub">{m.summary}</p>
        </div>
        <span className="status-text">Not built yet</span>
      </header>
      <section className={`panel risk-panel risk-${m.risk}`}>
        <div className="risk-head">
          <span className="dot" />
          {RISK_LABEL[m.risk]}
        </div>
        <p>{m.riskText}</p>
      </section>
      <section className="panel">
        <h2>Planned safeguards</h2>
        <ul className="checklist">
          {m.safeguards.map((s) => (
            <li key={s}>
              <Icon name="check" />
              {s}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
