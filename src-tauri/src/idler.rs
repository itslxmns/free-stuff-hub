//! Steam trading-card idling.
//!
//! - Card drops come from the user's own badges page on steamcommunity.com, read by a small
//!   script inside an app window where the user signed in on Steam's page. The app never sees
//!   the password or cookies; the script hands back only [appid, drops, hours, title] rows.
//! - "Running a game" = a helper process (`--idle <appid>`) that loads Steam's own
//!   steam_api64.dll, copied from a game the user already has installed. No game code runs.
//! - Safeguards: games under 2 h idle together (capped), then one at a time; random rotation
//!   and refresh times; pauses while a real game runs or Steam is closed; stop from the tray.

use crate::AppState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const BADGES_LABEL: &str = "steam-badges";
const UNDER_HOURS: f32 = 2.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleGame {
    pub appid: u32,
    pub title: String,
    pub drops: u32,
    pub hours: f32,
    pub idling: bool,
    /// Steam wouldn't start it (usually: not fully owned, e.g. Family Sharing or a free weekend).
    pub refused: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleView {
    pub running: bool,
    pub status: String,
    pub needs_login: bool,
    pub steam_ready: bool,
    pub dll_found: bool,
    pub games: Vec<IdleGame>,
    pub total_drops: u32,
    pub last_refresh: Option<DateTime<Utc>>,
    pub next_switch: Option<DateTime<Utc>>,
}

#[derive(Default)]
pub struct Idler {
    pub running: bool,
    epoch: u64,
    status: String,
    needs_login: bool,
    games: Vec<IdleGame>,
    children: BTreeMap<u32, Child>,
    last_refresh: Option<DateTime<Utc>>,
    next_switch: Option<DateTime<Utc>>,
    dll: Option<PathBuf>,
    /// App ids Steam refused during this run; cleared on every Start.
    refused: std::collections::BTreeSet<u32>,
    /// SteamID64 of the account whose badges page was read.
    community_sid: Option<u64>,
}

impl Idler {
    fn view(&self) -> IdleView {
        let mut games = self.games.clone();
        for g in &mut games {
            g.idling = self.children.contains_key(&g.appid);
            g.refused = self.refused.contains(&g.appid);
        }
        games.sort_by(|a, b| b.idling.cmp(&a.idling).then(a.refused.cmp(&b.refused)).then(b.drops.cmp(&a.drops)));
        IdleView {
            running: self.running,
            status: if self.status.is_empty() { "Stopped".into() } else { self.status.clone() },
            needs_login: self.needs_login,
            steam_ready: steam_signed_in(),
            dll_found: self.dll.is_some(),
            total_drops: games.iter().map(|g| g.drops).sum(),
            games,
            last_refresh: self.last_refresh,
            next_switch: self.next_switch,
        }
    }
}

// ------------------------------------------------------------------ Steam client state (registry)

fn steam_key(sub: &str) -> Option<winreg::RegKey> {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(format!("Software\\Valve\\Steam{sub}")).ok()
}

/// SteamID64 of the account signed in to the Steam client.
fn client_sid() -> Option<u64> {
    steam_key("\\ActiveProcess")
        .and_then(|k| k.get_value::<u32, _>("ActiveUser").ok())
        .filter(|u| *u != 0)
        .map(|u| 76_561_197_960_265_728 + u as u64)
}

pub fn steam_signed_in() -> bool {
    steam_key("\\ActiveProcess").and_then(|k| k.get_value::<u32, _>("ActiveUser").ok()).is_some_and(|u| u != 0)
}

fn running_app_id() -> u32 {
    steam_key("").and_then(|k| k.get_value::<u32, _>("RunningAppID").ok()).unwrap_or(0)
}

fn steam_path() -> Option<PathBuf> {
    steam_key("").and_then(|k| k.get_value::<String, _>("SteamPath").ok()).map(PathBuf::from)
}

// ------------------------------------------------------------------ steam_api64.dll

fn library_folders() -> Vec<PathBuf> {
    let Some(root) = steam_path() else { return vec![] };
    let mut out = vec![root.clone()];
    if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
        for line in vdf.lines() {
            let t = line.trim();
            if t.starts_with("\"path\"") {
                if let Some(p) = t.split('"').nth(3) {
                    let p = PathBuf::from(p.replace("\\\\", "\\"));
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

fn search(dir: &Path, depth: u32) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = vec![];
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("steam_api64.dll")) {
            return Some(p);
        }
        if p.is_dir() {
            subdirs.push(p);
        }
    }
    if depth == 0 {
        return None;
    }
    subdirs.into_iter().find_map(|d| search(&d, depth - 1))
}

/// Copy a steam_api64.dll from an installed game into the app's data folder (once).
fn ensure_dll(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("idle");
    let target = dir.join("steam_api64.dll");
    if target.exists() {
        return Ok(target);
    }
    let found = library_folders()
        .iter()
        .filter_map(|lib| std::fs::read_dir(lib.join("steamapps").join("common")).ok())
        .flat_map(|rd| rd.flatten().map(|e| e.path()))
        .find_map(|game| search(&game, 4))
        .ok_or("No steam_api64.dll found. Install any free Steam game that uses Steam features, then try again")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::copy(&found, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

// ------------------------------------------------------------------ helper processes

fn spawn_child(dll: &Path, appid: u32) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Command::new(exe)
        .arg("--idle")
        .arg(appid.to_string())
        .arg(dll)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())
}

fn stop_child(mut c: Child) {
    // Closing stdin asks the helper to shut Steam's API down cleanly.
    drop(c.stdin.take());
    for _ in 0..20 {
        if let Ok(Some(_)) = c.try_wait() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = c.kill();
}

fn stop_all(idler: &mut Idler) {
    for (_, c) in std::mem::take(&mut idler.children) {
        stop_child(c);
    }
}

// ------------------------------------------------------------------ badges page

/// Injected into the badges window. Only acts when opened with `#fsh-badges` (marker kept
/// 10 min across the sign-in round trip). Reports via a navigation the app intercepts.
const BADGES_SCRIPT: &str = r#"
(function () {
  if (location.hostname !== 'steamcommunity.com') return;
  var KEY = 'fsh-badges';
  if (location.hash === '#fsh-badges') sessionStorage.setItem(KEY, String(Date.now()));
  var marked = Date.now() - Number(sessionStorage.getItem(KEY) || 0) < 600000;
  if (!marked) return;
  var prof = location.pathname.match(/^\/(id|profiles)\/[^\/]+/);
  if (!prof) return;
  if (!/\/badges\/?$/.test(location.pathname)) {
    location.replace(prof[0] + '/badges/?l=english');
    return;
  }
  function parse(doc) {
    return Array.prototype.slice.call(doc.querySelectorAll('.badge_row')).map(function (r) {
      var a = r.querySelector('a.badge_row_overlay');
      var m = a && a.getAttribute('href').match(/gamecards\/(\d+)/);
      if (!m) return null;
      var p = r.querySelector('.progress_info_bold');
      var dm = p && p.textContent.match(/(\d+)\s+card drops? remaining/i);
      var h = r.querySelector('.badge_title_stats_playtime');
      var hm = h && h.textContent.replace(/,/g, '').match(/([\d.]+)\s*hrs?/i);
      var t = r.querySelector('.badge_title');
      var title = t ? (t.firstChild ? t.firstChild.textContent : t.textContent).replace(/\s+/g, ' ').trim() : '';
      return [Number(m[1]), dm ? Number(dm[1]) : 0, hm ? Number(hm[1]) : 0, title];
    }).filter(function (x) { return x && x[1] > 0; });
  }
  async function run() {
    var rows = parse(document);
    var pages = Array.prototype.slice.call(document.querySelectorAll('.pagelink')).map(function (x) { return Number(x.textContent); }).filter(function (n) { return n > 0; });
    var last = pages.length ? Math.min(Math.max.apply(null, pages), 30) : 1;
    for (var p = 2; p <= last; p++) {
      var html = await fetch(location.pathname + '?l=english&p=' + p, { credentials: 'same-origin' }).then(function (r) { return r.text(); });
      rows = rows.concat(parse(new DOMParser().parseFromString(html, 'text/html')));
    }
    sessionStorage.removeItem(KEY);
    var sid = String(window.g_steamID || '');
    location.href = 'https://steamcommunity.com/?fsh-badges=' + encodeURIComponent(JSON.stringify([sid, rows]));
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run); else run();
})();
"#;

fn on_badges_nav(app: &AppHandle, url: &tauri::Url) -> bool {
    if url.host_str() != Some("steamcommunity.com") {
        return true;
    }
    if url.path().starts_with("/login") {
        let st = app.state::<AppState>();
        let mut idler = st.idler.lock().unwrap();
        idler.needs_login = true;
        idler.status = "Sign in to Steam Community in the window that opened".into();
        drop(idler);
        if let Some(w) = app.get_webview_window(BADGES_LABEL) {
            let _ = w.show();
            let _ = w.set_focus();
        }
        emit(app);
        return true;
    }
    let Some(data) = url.query_pairs().find(|(k, _)| k == "fsh-badges").map(|(_, v)| v.to_string()) else {
        return true;
    };
    let (sid, rows): (String, Vec<(u32, u32, f32, String)>) = serde_json::from_str(&data).unwrap_or_default();
    apply_badges(app, &sid, rows);
    if let Some(w) = app.get_webview_window(BADGES_LABEL) {
        let _ = w.close();
    }
    false // the report URL itself never loads
}

fn apply_badges(app: &AppHandle, community_sid: &str, rows: Vec<(u32, u32, f32, String)>) {
    let st = app.state::<AppState>();
    let mut logs = vec![];
    {
        let mut idler = st.idler.lock().unwrap();
        idler.community_sid = community_sid.parse().ok();
        for old in &idler.games {
            let now = rows.iter().find(|r| r.0 == old.appid).map(|r| r.1).unwrap_or(0);
            if now < old.drops {
                let got = old.drops - now;
                logs.push(if now == 0 {
                    format!("Card drops done for {} (+{got})", old.title)
                } else {
                    format!("Card dropped: {} (+{got}, {now} left)", old.title)
                });
            }
        }
        idler.games = rows
            .into_iter()
            .map(|(appid, drops, hours, title)| IdleGame { appid, title, drops, hours, idling: false, refused: false })
            .collect();
        idler.last_refresh = Some(Utc::now());
        idler.needs_login = false;
    }
    if !logs.is_empty() {
        let mut store = st.store.lock().unwrap();
        for l in logs {
            store.log("steam-idle", "reward", l);
        }
        store.save();
    }
    emit(app);
}

/// Open (or reuse) the badges window. Hidden unless Steam asks the user to sign in.
fn open_badges(app: &AppHandle, visible: bool) -> Result<(), String> {
    let url: tauri::Url = "https://steamcommunity.com/my/badges/?l=english#fsh-badges".parse().unwrap();
    if let Some(w) = app.get_webview_window(BADGES_LABEL) {
        w.navigate(url).map_err(|e| e.to_string())?;
        if visible {
            let _ = w.show();
            let _ = w.set_focus();
        }
        return Ok(());
    }
    let handle = app.clone();
    WebviewWindowBuilder::new(app, BADGES_LABEL, WebviewUrl::External(url))
        .title("Steam Community - sign in for card idling")
        .inner_size(1000.0, 760.0)
        .visible(visible)
        .initialization_script(BADGES_SCRIPT)
        .on_navigation(move |u| on_badges_nav(&handle, u))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ scheduler

fn emit(app: &AppHandle) {
    let view = app.state::<AppState>().idler.lock().unwrap().view();
    let _ = app.emit("idle", view);
}

fn set_status(app: &AppHandle, s: &str) {
    app.state::<AppState>().idler.lock().unwrap().status = s.into();
    emit(app);
}

fn log(app: &AppHandle, level: &str, msg: String) {
    let st = app.state::<AppState>();
    let mut store = st.store.lock().unwrap();
    store.log("steam-idle", level, msg);
    store.save();
}

fn alive(app: &AppHandle, epoch: u64) -> bool {
    let st = app.state::<AppState>();
    let idler = st.idler.lock().unwrap();
    idler.running && idler.epoch == epoch
}

async fn sleep_secs(s: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(s)).await;
}

/// Random value in [min, max] seconds.
fn rand_between(min: i64, max: i64) -> u64 {
    let span = (max - min).max(1);
    (min + (crate::jitter_secs(span) + span).rem_euclid(span + 1)) as u64
}

async fn refresh(app: &AppHandle, epoch: u64) -> bool {
    let before = app.state::<AppState>().idler.lock().unwrap().last_refresh;
    if open_badges(app, false).is_err() {
        return false;
    }
    for _ in 0..60 {
        sleep_secs(2).await;
        if !alive(app, epoch) {
            return false;
        }
        let (done, needs_login) = {
            let st = app.state::<AppState>();
            let idler = st.idler.lock().unwrap();
            (idler.last_refresh != before, idler.needs_login)
        };
        if done {
            return true;
        }
        if needs_login {
            // Wait (up to 10 min) for the user to sign in; the window then reports by itself.
            for _ in 0..300 {
                sleep_secs(2).await;
                if !alive(app, epoch) {
                    return false;
                }
                if app.state::<AppState>().idler.lock().unwrap().last_refresh != before {
                    return true;
                }
            }
            return false;
        }
    }
    false
}

fn run_loop(app: AppHandle, epoch: u64) {
    tauri::async_runtime::spawn(async move {
        let dll = match ensure_dll(&app) {
            Ok(d) => d,
            Err(e) => {
                let st = app.state::<AppState>();
                let mut idler = st.idler.lock().unwrap();
                idler.running = false;
                idler.status = e.clone();
                drop(idler);
                log(&app, "error", e);
                emit(&app);
                return;
            }
        };
        app.state::<AppState>().idler.lock().unwrap().dll = Some(dll.clone());
        log(&app, "info", "Card idling started".into());

        while alive(&app, epoch) {
            // 1. Steam must be running and signed in.
            if !steam_signed_in() {
                stop_all(&mut app.state::<AppState>().idler.lock().unwrap());
                set_status(&app, "Waiting for Steam: open Steam and sign in");
                sleep_secs(30).await;
                continue;
            }
            // 2. Pause while the user plays a real game.
            let ours: Vec<u32> = app.state::<AppState>().idler.lock().unwrap().children.keys().copied().collect();
            let rid = running_app_id();
            if rid != 0 && !ours.contains(&rid) {
                stop_all(&mut app.state::<AppState>().idler.lock().unwrap());
                set_status(&app, "Paused while you play a game");
                sleep_secs(30).await;
                continue;
            }
            // 3. Read card drops from the badges page.
            set_status(&app, "Reading card drops from Steam");
            if !refresh(&app, epoch).await {
                if alive(&app, epoch) {
                    set_status(&app, "Couldn't read your badges page. Retrying in a few minutes");
                    sleep_secs(rand_between(120, 240)).await;
                }
                continue;
            }
            // Badges and the Steam client must be the same account, or Steam refuses every game.
            let community = app.state::<AppState>().idler.lock().unwrap().community_sid;
            if let (Some(c), Some(cl)) = (community, client_sid()) {
                if c != cl {
                    let st = app.state::<AppState>();
                    let mut idler = st.idler.lock().unwrap();
                    stop_all(&mut idler);
                    idler.running = false;
                    idler.next_switch = None;
                    idler.status = "Steam Community and the Steam client are signed in to different accounts. Switch one so they match, then start again".into();
                    drop(idler);
                    log(&app, "error", format!("Card idling stopped: badges are from account {c}, but the Steam client is on {cl}"));
                    emit(&app);
                    break;
                }
            }
            // 4. Pick what to idle.
            let (max_games, multi) = {
                let st = app.state::<AppState>();
                let s = &st.store.lock().unwrap().data.settings;
                (s.idle_max_games.clamp(1, 32) as usize, s.idle_multi)
            };
            let (all_games, refused) = {
                let st = app.state::<AppState>();
                let idler = st.idler.lock().unwrap();
                (idler.games.clone(), idler.refused.clone())
            };
            let games: Vec<IdleGame> = all_games.iter().filter(|g| !refused.contains(&g.appid)).cloned().collect();
            if games.is_empty() && !all_games.is_empty() {
                let st = app.state::<AppState>();
                let mut idler = st.idler.lock().unwrap();
                stop_all(&mut idler);
                idler.running = false;
                idler.next_switch = None;
                idler.status = "Steam won't idle the games that have drops left. They may not be fully owned on this account".into();
                drop(idler);
                log(&app, "error", "Card idling stopped: Steam refused every game with drops left".into());
                emit(&app);
                break;
            }
            if games.is_empty() {
                let st = app.state::<AppState>();
                let mut idler = st.idler.lock().unwrap();
                stop_all(&mut idler);
                idler.running = false;
                idler.next_switch = None;
                idler.status = "All card drops collected".into();
                drop(idler);
                log(&app, "reward", "Card idling finished: no drops left".into());
                emit(&app);
                break;
            }
            let mut under: Vec<&IdleGame> = games.iter().filter(|g| g.hours < UNDER_HOURS).collect();
            under.sort_by(|a, b| b.hours.partial_cmp(&a.hours).unwrap_or(std::cmp::Ordering::Equal));
            let (targets, period, label): (Vec<u32>, u64, String) = if multi && !under.is_empty() {
                let t: Vec<u32> = under.iter().take(max_games).map(|g| g.appid).collect();
                let n = t.len();
                (t, rand_between(20 * 60, 30 * 60), format!("Building playtime on {n} game{} at once", if n == 1 { "" } else { "s" }))
            } else {
                let g = games.iter().max_by_key(|g| g.drops).unwrap();
                (vec![g.appid], rand_between(15 * 60, 25 * 60), format!("Idling {} for card drops", g.title))
            };

            // 5. Start/stop helpers to match, with small random gaps between starts.
            {
                let st = app.state::<AppState>();
                let mut idler = st.idler.lock().unwrap();
                let stale: Vec<u32> = idler.children.keys().filter(|a| !targets.contains(a)).copied().collect();
                for a in stale {
                    if let Some(c) = idler.children.remove(&a) {
                        stop_child(c);
                    }
                }
            }
            for a in &targets {
                if app.state::<AppState>().idler.lock().unwrap().children.contains_key(a) {
                    continue;
                }
                match spawn_child(&dll, *a) {
                    Ok(c) => {
                        app.state::<AppState>().idler.lock().unwrap().children.insert(*a, c);
                    }
                    Err(e) => log(&app, "error", format!("Couldn't start idling {a}: {e}")),
                }
                sleep_secs(rand_between(2, 5)).await;
            }
            {
                let st = app.state::<AppState>();
                let mut idler = st.idler.lock().unwrap();
                idler.next_switch = Some(Utc::now() + chrono::Duration::seconds(period as i64));
                idler.status = label;
            }
            emit(&app);

            // 6. Wait out the period, watching for stop, Steam closing, real games and helper exits.
            let mut waited = 0;
            while waited < period && alive(&app, epoch) {
                sleep_secs(15).await;
                waited += 15;
                let rid = running_app_id();
                let st = app.state::<AppState>();
                let mut idler = st.idler.lock().unwrap();
                let mut failed = vec![];
                for (a, c) in idler.children.iter_mut() {
                    if let Ok(Some(code)) = c.try_wait() {
                        failed.push((*a, code.code().unwrap_or(-1)));
                    }
                }
                for (a, code) in &failed {
                    idler.children.remove(a);
                    idler.refused.insert(*a);
                    let title = idler.games.iter().find(|g| g.appid == *a).map(|g| g.title.clone()).unwrap_or(a.to_string());
                    drop(idler);
                    let why = if *code == 2 { "Steam wouldn't start it, usually because the game isn't fully owned on this account" } else { "the helper stopped unexpectedly" };
                    log(&app, "error", format!("Skipping {title} for card idling: {why} (code {code})"));
                    idler = st.idler.lock().unwrap();
                }
                let ours: Vec<u32> = idler.children.keys().copied().collect();
                drop(idler);
                if !failed.is_empty() {
                    emit(&app);
                }
                if !steam_signed_in() || (rid != 0 && !ours.contains(&rid)) || ours.is_empty() {
                    break;
                }
            }
        }
        let st = app.state::<AppState>();
        let mut idler = st.idler.lock().unwrap();
        if idler.epoch == epoch {
            stop_all(&mut idler);
            idler.next_switch = None;
        }
        drop(idler);
        emit(&app);
    });
}

// ------------------------------------------------------------------ commands

#[tauri::command]
pub fn idle_state(state: tauri::State<AppState>) -> IdleView {
    state.idler.lock().unwrap().view()
}

#[tauri::command]
pub fn idle_start(app: AppHandle, state: tauri::State<AppState>) -> IdleView {
    let epoch = {
        let mut idler = state.idler.lock().unwrap();
        if idler.running {
            return idler.view();
        }
        idler.running = true;
        idler.epoch += 1;
        idler.refused.clear();
        idler.status = "Starting".into();
        idler.epoch
    };
    run_loop(app, epoch);
    state.idler.lock().unwrap().view()
}

#[tauri::command]
pub fn idle_stop(app: AppHandle, state: tauri::State<AppState>) -> IdleView {
    stop(&app);
    state.idler.lock().unwrap().view()
}

/// Show the Steam Community window (sign in, or just refresh the list now).
#[tauri::command]
pub async fn idle_sign_in(app: AppHandle) -> Result<(), String> {
    open_badges(&app, true)
}

pub fn stop(app: &AppHandle) {
    let st = app.state::<AppState>();
    let mut idler = st.idler.lock().unwrap();
    let was = idler.running;
    idler.running = false;
    idler.epoch += 1;
    stop_all(&mut idler);
    idler.status = "Stopped".into();
    idler.next_switch = None;
    drop(idler);
    if was {
        log(app, "info", "Card idling stopped".into());
    }
    emit(app);
}

/// Called on app exit so no helper keeps Steam "in game".
pub fn shutdown(app: &AppHandle) {
    let st = app.state::<AppState>();
    let mut idler = st.idler.lock().unwrap();
    idler.running = false;
    stop_all(&mut idler);
}
