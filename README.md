# Free Stuff Hub

A Windows tray app that gathers free game rewards in one dashboard.
Built with **Tauri 2** (Rust backend) and **React + TypeScript**.

| Module | Status | ToS risk |
| --- | --- | --- |
| **Free games**: Epic Games Store, Steam 100% off (with Add to account), GOG giveaways, Prime Gaming reminders | Working | Low |
| Steam trading-card idling | Planned | Low |
| Twitch & Kick drops | Planned | Medium |
| Discord quests | Planned | **High** |

## Read this first

Some planned modules automate things that platforms forbid. Spoofing Discord quests and
auto-watching Twitch/Kick streams **break those services' Terms of Service**. Rewards can be
revoked and accounts can be flagged or restricted. Those modules will ship with safeguards
(one task at a time, random timing, daily caps, a kill switch), but you take the risk.

**Free Stuff Hub never asks for your passwords or tokens.** Every sign-in happens on the
platform's official page, either in an app window (with no access to the app's internals) or in
your browser.

## Free games module

- Checks the public Epic free-games feed and the GOG catalog and homepage on a schedule
  (every 1-24 h, plus or minus 20 min of random jitter) and from the tray.
- Shows what's **free now**, **upcoming** (Epic's next week) and a **monthly Prime Gaming**
  reminder, since Prime has no public feed.
- Sends a desktop notification when a new free game appears.
- **Claim** opens the official store page in an app window, where you sign in and click
  "Get" yourself. **Mark claimed** tracks what you've already grabbed.
- State is saved to `%APPDATA%\com.freestuffhub.app\state.json`.

## Steam: 100% off games

- Detects Steam games that are temporarily **100% off** (free to keep), not regular free-to-play games.
- **Add to account** opens the game's official Steam page in an app window and clicks Steam's own
  "Add to Account" button for you after a short random delay. The first time, sign in on Steam's
  login page (Steam Guard works as normal); the add finishes after you sign in.
- **Auto-add** (Settings, off by default) adds new free Steam games by itself in a hidden window:
  one at a time, with random gaps, up to a daily limit (default 10).
- Safety: this is the same request Steam's button sends when you click it. Tools like
  ArchiSteamFarm have done this for years without bans. Steam itself limits accounts to about 50
  free licenses per hour, far above what this app does. The app never sees your Steam password,
  cookies or Steam Guard codes.

## Install

**Easiest:** download `Free Stuff Hub_x.y.z_x64-setup.exe` from
[Releases](https://github.com/itslxmns/free-stuff-hub/releases/latest) and run it.

**From source:** double-click `setup.cmd`. It starts the app if it's already installed. Otherwise
it checks for Node.js and Rust, builds the app and installs it (`setup.cmd --rebuild` forces a
fresh build).

The installer needs no admin rights. It adds Start menu and
desktop shortcuts. In **Settings** you can turn on **Start with Windows**, which starts the app
hidden in the tray when you log in.

## Development

Requirements: Node 20+, Rust (stable, MSVC), WebView2 (included with Windows 11).

```powershell
npm install
npm run tauri dev      # run the app
cd src-tauri; cargo test   # backend unit tests
npm run tauri build    # installer in src-tauri/target/release/bundle
```

## Layout

```
src/                 React dashboard
  modules/           one file per module (FreeGames, Planned...)
  lib/api.ts         typed wrappers around the Tauri commands
src-tauri/src/
  lib.rs             tray, background poller, commands, store windows
  free_games.rs      Epic / GOG / Prime sources (+ tests)
  state.rs           persisted settings, claimed list, activity log
```
