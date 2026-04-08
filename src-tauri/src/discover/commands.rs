use crate::discover::helpers::mc_news::{fetch_mc_news_page, MC_NEWS_ENDPOINT};
use crate::discover::models::{NewsPostRequest, NewsPostResponse, NewsSourceInfo};
use crate::error::{SJMCLError, SJMCLResult};
use crate::launcher_config::models::LauncherConfig;
use crate::resource::models::McmodRankingItem;
use crate::utils::web::with_retry;
use futures::future;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_http::reqwest;

#[tauri::command]
pub async fn fetch_news_sources_info(app: AppHandle) -> SJMCLResult<Vec<NewsSourceInfo>> {
  let post_sources = {
    let binding = app.state::<Mutex<LauncherConfig>>();
    let state = binding.lock().unwrap();
    state.discover_source_endpoints.clone()
  };

  let client = with_retry(app.state::<reqwest::Client>().inner().clone());

  let tasks: Vec<_> = post_sources
    .into_iter()
    .map(|(url, _)| {
      let client = client.clone();
      async move {
        let mut news_source = NewsSourceInfo {
          name: "".to_string(),
          full_name: "".to_string(),
          endpoint_url: url.clone(),
          icon_src: "".to_string(),
        };

        let response = client
          .get(&url)
          .query(&[("pageSize", "0")]) // ?pageSize=0
          .send()
          .await;

        if let Ok(response) = response {
          let json_data: serde_json::Value = response.json().await.unwrap_or_default();

          if let Some(source_info) = json_data.get("sourceInfo") {
            news_source.name = source_info["name"].as_str().unwrap_or("").to_string();
            news_source.full_name = source_info["fullName"].as_str().unwrap_or("").to_string();
            news_source.icon_src = source_info["iconSrc"].as_str().unwrap_or("").to_string();
          }
        }

        news_source
      }
    })
    .collect();

  Ok(future::join_all(tasks).await)
}

#[tauri::command]
pub async fn fetch_news_post_summaries(
  app: AppHandle,
  requests: Vec<NewsPostRequest>,
) -> SJMCLResult<NewsPostResponse> {
  let client = with_retry(app.state::<reqwest::Client>().inner().clone());
  let tasks: Vec<_> = requests
    .into_iter()
    .map(|NewsPostRequest { url, cursor }| {
      let client = client.clone();
      async move {
        if url.starts_with(MC_NEWS_ENDPOINT) {
          return fetch_mc_news_page(&client, &url, cursor).await;
        }

        let mut req = client.get(&url).query(&[("pageSize", "12")]);

        if let Some(c) = cursor {
          req = req.query(&[("cursor", &c.to_string())]);
        }

        let resp = req.send().await;
        match resp {
          Ok(resp) if resp.status().is_success() => {
            let parsed: Result<NewsPostResponse, _> = resp.json().await;
            parsed.ok().map(|mut p| {
              for post in &mut p.posts {
                post.source.endpoint_url = url.clone();
              }
              (url.clone(), p)
            })
          }
          _ => None,
        }
      }
    })
    .collect();

  let results = futures::future::join_all(tasks).await;

  let mut all_posts = Vec::new();
  let mut cursors_map = HashMap::new();

  for result in results.into_iter().flatten() {
    let (url, post_response) = result;
    all_posts.extend(post_response.posts);
    if let Some(next_cursor) = post_response.next {
      cursors_map.insert(url, next_cursor);
    }
  }

  all_posts.sort_by(|a, b| b.create_at.cmp(&a.create_at));

  Ok(NewsPostResponse {
    posts: all_posts,
    next: None,
    cursors: Some(cursors_map),
  })
}

#[tauri::command]
pub async fn fetch_mcmod_rankings(
  app: AppHandle,
  category: String,
  period: String,
) -> SJMCLResult<Vec<McmodRankingItem>> {
  let category_id = match category.as_str() {
    "technology" => 1,
    "magic" => 2,
    "adventure" => 3,
    "farming" => 4,
    "decoration" => 5,
    "utility" => 6,
    "auxiliary" => 7,
    "lib" => 8,
    _ => 1,
  };

  let period_id = match period.as_str() {
    "day" => 1,
    "week" => 2,
    "month" => 3,
    _ => 1,
  };

  let url = format!(
    "https://www.mcmod.cn/class/category/{}-{}.html",
    category_id, period_id
  );

  let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()
    .map_err(|e| SJMCLError(format!("Failed to create HTTP client: {}", e)))?;

  let response = client.get(&url).send().await?;

  if !response.status().is_success() {
    return Err(SJMCLError("Failed to fetch rankings page".to_string()));
  }

  let html = response.text().await?;

  let period_id_name = match period.as_str() {
    "day" => "day",
    "week" => "week",
    "month" => "moon",
    _ => "day",
  };

  let list_re = Regex::new(&format!(
    r#"(?is)<ul[^>]*\bid=['"]{}['"][^>]*>(.*?)</ul>"#,
    period_id_name
  ))?;
  let link_re = Regex::new(r#"(?is)<a[^>]*href=['"]/class/(\d+)\.html['"][^>]*>([^<]+)</a>"#)?;

  let mut rankings = Vec::new();

  if let Some(list_caps) = list_re.captures(&html) {
    let list_html = list_caps.get(1).map(|m| m.as_str()).unwrap_or("");
    for cap in link_re.captures_iter(list_html) {
      if rankings.len() >= 10 {
        break;
      }
      if let Some(id_match) = cap.get(1) {
        if let Ok(mcmod_id) = id_match.as_str().parse::<u32>() {
          let name = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
          if !name.is_empty() {
            rankings.push(McmodRankingItem {
              id: format!("mcmod-{}", mcmod_id),
              name: name.to_string(),
              website_url: format!("https://www.mcmod.cn/class/{}.html", mcmod_id),
            });
          }
        }
      }
    }
  }

  Ok(rankings)
}
