//! Free-game sources: Epic Games Store, GOG, Prime Gaming.
//!
//! Only public, unauthenticated endpoints are used here. Claiming always happens
//! on the store's official page, inside a window where the user signs in themselves:
//! this app never sees passwords or tokens.

use chrono::{DateTime, Datelike, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) FreeStuffHub/0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Store {
    Epic,
    Gog,
    Prime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OfferStatus {
    /// Free right now, claimable.
    Active,
    /// Announced, becomes free later.
    Upcoming,
    /// No public feed (Prime Gaming): a recurring "go check" reminder.
    Reminder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeGame {
    pub id: String,
    pub store: Store,
    pub title: String,
    pub url: String,
    pub image: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub status: OfferStatus,
    pub note: Option<String>,
}

pub struct FetchResult {
    pub games: Vec<FreeGame>,
    pub errors: Vec<String>,
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("http client")
}

pub async fn fetch_all(epic: bool, gog: bool, prime: bool) -> FetchResult {
    let http = client();
    let mut games = Vec::new();
    let mut errors = Vec::new();

    if epic {
        match fetch_epic(&http).await {
            Ok(mut g) => games.append(&mut g),
            Err(e) => errors.push(format!("Epic: {e}")),
        }
    }
    if gog {
        match fetch_gog(&http).await {
            Ok(mut g) => games.append(&mut g),
            Err(e) => errors.push(format!("GOG: {e}")),
        }
    }
    if prime {
        games.push(prime_reminder(Utc::now()));
    }
    FetchResult { games, errors }
}

// ---------------------------------------------------------------- Epic

pub async fn fetch_epic(http: &reqwest::Client) -> Result<Vec<FreeGame>, String> {
    let url = "https://store-site-backend-static.ak.epicgames.com/freeGamesPromotions?locale=en-US&country=US&allowCountries=US";
    let v: Value = http
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(parse_epic(&v, Utc::now()))
}

fn parse_epic(v: &Value, now: DateTime<Utc>) -> Vec<FreeGame> {
    let mut out = Vec::new();
    let Some(elements) = v.pointer("/data/Catalog/searchStore/elements").and_then(Value::as_array) else {
        return out;
    };
    for e in elements {
        let Some(promos) = e.get("promotions").filter(|p| !p.is_null()) else { continue };
        let pick = |key: &str| -> Option<(DateTime<Utc>, DateTime<Utc>)> {
            promos.get(key)?.as_array()?.iter().flat_map(|g| {
                g.get("promotionalOffers").and_then(Value::as_array).cloned().unwrap_or_default()
            }).find_map(|o| {
                let pct = o.pointer("/discountSetting/discountPercentage")?.as_i64()?;
                if pct != 0 { return None; }
                let s = o.get("startDate")?.as_str()?.parse().ok()?;
                let en = o.get("endDate")?.as_str()?.parse().ok()?;
                Some((s, en))
            })
        };
        let (status, window) = if let Some(w) = pick("promotionalOffers") {
            if w.0 <= now && now < w.1 { (OfferStatus::Active, w) } else { continue }
        } else if let Some(w) = pick("upcomingPromotionalOffers") {
            (OfferStatus::Upcoming, w)
        } else {
            continue;
        };

        let title = e.get("title").and_then(Value::as_str).unwrap_or("Unknown game").to_string();
        let id = e.get("id").and_then(Value::as_str).unwrap_or(&title).to_string();
        let slug = e.pointer("/catalogNs/mappings").and_then(Value::as_array)
            .and_then(|m| m.iter().find_map(|x| x.get("pageSlug").and_then(Value::as_str)))
            .or_else(|| e.get("productSlug").and_then(Value::as_str).filter(|s| !s.is_empty()))
            .or_else(|| e.pointer("/offerMappings/0/pageSlug").and_then(Value::as_str))
            .map(|s| s.trim_end_matches("/home").to_string());
        let url = match slug {
            Some(s) => format!("https://store.epicgames.com/en-US/p/{s}"),
            None => "https://store.epicgames.com/en-US/free-games".to_string(),
        };
        let image = e.get("keyImages").and_then(Value::as_array).and_then(|imgs| {
            ["OfferImageWide", "DieselStoreFrontWide", "Thumbnail"].iter().find_map(|t| {
                imgs.iter().find(|i| i.get("type").and_then(Value::as_str) == Some(t))
                    .and_then(|i| i.get("url")).and_then(Value::as_str).map(String::from)
            })
        });
        out.push(FreeGame {
            id: format!("epic:{id}:{}", window.0.format("%Y%m%d")),
            store: Store::Epic,
            title,
            url,
            image,
            starts_at: Some(window.0),
            ends_at: Some(window.1),
            status,
            note: None,
        });
    }
    out
}

// ---------------------------------------------------------------- GOG

pub async fn fetch_gog(http: &reqwest::Client) -> Result<Vec<FreeGame>, String> {
    // 1) Catalog: games discounted to $0 (time-limited giveaways show up here).
    let url = "https://catalog.gog.com/v1/catalog?limit=48&price=between:0,0&discounted=eq:true&productType=in:game,pack&order=desc:trending";
    let v: Value = http.get(url).send().await.map_err(|e| e.to_string())?
        .error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    let mut out = parse_gog_catalog(&v);

    // 2) Homepage giveaway banner (best effort; markup may change).
    if let Ok(resp) = http.get("https://www.gog.com/en/").send().await {
        if let Ok(html) = resp.text().await {
            for g in parse_gog_homepage(&html) {
                if !out.iter().any(|o| o.url == g.url) {
                    out.push(g);
                }
            }
        }
    }
    Ok(out)
}

fn parse_gog_catalog(v: &Value) -> Vec<FreeGame> {
    let Some(products) = v.get("products").and_then(Value::as_array) else { return vec![] };
    products.iter().filter_map(|p| {
        let id = p.get("id")?.as_str()?.to_string();
        let title = p.get("title")?.as_str()?.to_string();
        let slug = p.get("slug").and_then(Value::as_str).unwrap_or_default();
        let url = p.get("storeLink").and_then(Value::as_str).map(String::from)
            .unwrap_or_else(|| format!("https://www.gog.com/en/game/{slug}"));
        Some(FreeGame {
            id: format!("gog:{id}"),
            store: Store::Gog,
            title,
            url,
            image: p.get("coverHorizontal").and_then(Value::as_str).map(String::from),
            starts_at: None,
            ends_at: None,
            status: OfferStatus::Active,
            note: Some("100% off on GOG. Claim it while the deal lasts.".into()),
        })
    }).collect()
}

fn parse_gog_homepage(html: &str) -> Vec<FreeGame> {
    // Look for an element whose class or id mentions "giveaway", then the first game link after it.
    let re = Regex::new(r#"(?s)<[a-z-]+[^>]*(?:class|id)="[^"]*giveaway[^"]*"[^>]*>.{0,4000}?href="(https://www\.gog\.com/(?:[a-z]{2}/)?game/([a-z0-9_]+))""#).unwrap();
    re.captures_iter(html).take(3).map(|c| {
        let slug = c[2].to_string();
        let title = slug.split('_').map(|w| {
            let mut ch = w.chars();
            ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
        }).collect::<Vec<_>>().join(" ");
        FreeGame {
            id: format!("gog-giveaway:{slug}"),
            store: Store::Gog,
            title,
            url: "https://www.gog.com/en/#giveaway".into(),
            image: None,
            starts_at: None,
            ends_at: None,
            status: OfferStatus::Active,
            note: Some(format!("GOG homepage giveaway ({}). Claim it from the banner on gog.com.", &c[1])),
        }
    }).collect()
}

// ---------------------------------------------------------------- Prime Gaming

/// Prime Gaming (now under Amazon Luna "claims") has no public feed: keep a monthly reminder.
pub fn prime_reminder(now: DateTime<Utc>) -> FreeGame {
    FreeGame {
        id: format!("prime:{}-{:02}", now.year(), now.month()),
        store: Store::Prime,
        title: format!("Prime Gaming games for {}", now.format("%B %Y")),
        url: "https://gaming.amazon.com/home".into(),
        image: None,
        starts_at: None,
        ends_at: None,
        status: OfferStatus::Reminder,
        note: Some("Needs an Amazon Prime membership. Open the claims page, sign in, and claim this month's games.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epic_parses_active_and_upcoming() {
        let now: DateTime<Utc> = "2026-11-20T00:00:00Z".parse().unwrap();
        let v: Value = serde_json::json!({"data":{"Catalog":{"searchStore":{"elements":[
          {"title":"Free Now","id":"a","catalogNs":{"mappings":[{"pageSlug":"free-now"}]},"keyImages":[{"type":"OfferImageWide","url":"http://img"}],
           "promotions":{"promotionalOffers":[{"promotionalOffers":[{"startDate":"2026-11-19T16:00:00.000Z","endDate":"2026-11-26T16:00:00.000Z","discountSetting":{"discountPercentage":0}}]}],"upcomingPromotionalOffers":[]}},
          {"title":"Soon","id":"b","promotions":{"promotionalOffers":[],"upcomingPromotionalOffers":[{"promotionalOffers":[{"startDate":"2026-11-26T16:00:00.000Z","endDate":"2026-12-03T16:00:00.000Z","discountSetting":{"discountPercentage":0}}]}]}},
          {"title":"Discount only","id":"c","promotions":{"promotionalOffers":[{"promotionalOffers":[{"startDate":"2026-11-19T16:00:00.000Z","endDate":"2026-11-26T16:00:00.000Z","discountSetting":{"discountPercentage":50}}]}]}},
          {"title":"No promo","id":"d","promotions":null}
        ]}}}});
        let g = parse_epic(&v, now);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].status, OfferStatus::Active);
        assert_eq!(g[0].url, "https://store.epicgames.com/en-US/p/free-now");
        assert_eq!(g[0].image.as_deref(), Some("http://img"));
        assert_eq!(g[1].status, OfferStatus::Upcoming);
    }

    #[test]
    fn gog_homepage_detects_giveaway() {
        let html = r#"<div class="giveaway-banner"><a href="https://www.gog.com/en/game/some_cool_game">x</a></div>"#;
        let g = parse_gog_homepage(html);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].title, "Some Cool Game");
    }

    #[test]
    fn prime_reminder_is_monthly() {
        let now: DateTime<Utc> = "2026-11-20T00:00:00Z".parse().unwrap();
        assert_eq!(prime_reminder(now).id, "prime:2026-11");
    }
}
