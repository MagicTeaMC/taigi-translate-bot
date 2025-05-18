use std::env;

use dotenv::dotenv;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use reqwest;
use scraper::{Html, Selector};
use serde_json::Value;
use urlencoding;

struct Handler;

async fn search_taigitv(keyword: &str) -> Result<Vec<String>, String> {
    let search_url = format!(
        "https://www.taigitv.org.tw/taigi-words?keyword={}",
        urlencoding::encode(keyword)
    );

    let response_text = match reqwest::get(&search_url).await {
        Ok(response) => match response.text().await {
            Ok(text) => text,
            Err(_) => return Err("Error reading response from TaigiTV".to_string()),
        },
        Err(_) => return Err("Error fetching from TaigiTV".to_string()),
    };

    // Parse HTML document
    let document = Html::parse_document(&response_text);

    // Fixed selectors for TaigiTV
    let link_selector = Selector::parse(".btngaa .h3 a")
        .map_err(|_| "Could not parse TaigiTV selector".to_string())?;

    // Extract results
    let results: Vec<String> = document
        .select(&link_selector)
        .filter_map(|element| {
            let text = element.text().collect::<String>().trim().to_string();
            let url = element.value().attr("href").map(|href| {
                if href.starts_with("http") {
                    href.to_string()
                } else if href.starts_with("/") {
                    format!("https://www.taigitv.org.tw{}", href)
                } else {
                    format!("https://www.taigitv.org.tw/{}", href)
                }
            });

            url.map(|u| format!("📺 {} - {}", text, u))
        })
        .take(3) // Limit to 3 results from TaigiTV
        .collect();

    Ok(results)
}

async fn search_sutian(keyword: &str) -> Result<Vec<String>, String> {
    let search_url = format!(
        "https://sutian.moe.edu.tw/zh-hant/tshiau/?lui=hua_su&tsha={}",
        urlencoding::encode(keyword)
    );

    let response_text = match reqwest::get(&search_url).await {
        Ok(response) => match response.text().await {
            Ok(text) => text,
            Err(_) => return Err("Error reading response from Sutian".to_string()),
        },
        Err(_) => return Err("Error fetching from Sutian".to_string()),
    };

    // Parse HTML document
    let document = Html::parse_document(&response_text);

    // Selectors for Sutian - extracting from both mobile and desktop tables
    let mobile_link_selector = Selector::parse("table.d-md-none tbody tr:nth-child(2) td a")
        .map_err(|_| "Could not parse Sutian mobile selector".to_string())?;
    let desktop_link_selector =
        Selector::parse("table.d-none.d-md-table tbody tr td:nth-child(2) a")
            .map_err(|_| "Could not parse Sutian desktop selector".to_string())?;

    let mobile_pronunciation_selector = Selector::parse("table.d-md-none tbody tr:nth-child(3) td")
        .map_err(|_| "Could not parse Sutian mobile pronunciation selector".to_string())?;
    let desktop_pronunciation_selector =
        Selector::parse("table.d-none.d-md-table tbody tr td:nth-child(3)")
            .map_err(|_| "Could not parse Sutian desktop pronunciation selector".to_string())?;

    let mut results = Vec::new();

    // Try mobile table first
    if let (Some(link_element), Some(pronunciation_element)) = (
        document.select(&mobile_link_selector).next(),
        document.select(&mobile_pronunciation_selector).next(),
    ) {
        let word = link_element.text().collect::<String>().trim().to_string();
        let href = link_element.value().attr("href").unwrap_or("");
        let pronunciation = pronunciation_element
            .text()
            .collect::<String>()
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        let full_url = if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with("/") {
            format!("https://sutian.moe.edu.tw{}", href)
        } else {
            format!("https://sutian.moe.edu.tw/{}", href)
        };

        if !word.is_empty() && !pronunciation.is_empty() {
            results.push(format!("📚 {} [{}] - {}", word, pronunciation, full_url));
        }
    }
    // If no mobile results, try desktop table
    else if let (Some(link_element), Some(pronunciation_element)) = (
        document.select(&desktop_link_selector).next(),
        document.select(&desktop_pronunciation_selector).next(),
    ) {
        let word = link_element.text().collect::<String>().trim().to_string();
        let href = link_element.value().attr("href").unwrap_or("");
        let pronunciation = pronunciation_element
            .text()
            .collect::<String>()
            .trim()
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        let full_url = if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with("/") {
            format!("https://sutian.moe.edu.tw{}", href)
        } else {
            format!("https://sutian.moe.edu.tw/{}", href)
        };

        if !word.is_empty() && !pronunciation.is_empty() {
            results.push(format!("📚 {} [{}] - {}", word, pronunciation, full_url));
        }
    }

    Ok(results)
}

async fn search_itaigi(keyword: &str) -> Result<Vec<String>, String> {
    let search_url = format!(
        "https://itaigi.tw/平臺項目列表/揣列表?關鍵字={}",
        urlencoding::encode(keyword)
    );

    let response_text = match reqwest::get(&search_url).await {
        Ok(response) => match response.text().await {
            Ok(text) => text,
            Err(_) => return Err("Error reading response from iTaigi".to_string()),
        },
        Err(_) => return Err("Error fetching from iTaigi".to_string()),
    };

    // Parse JSON response
    let json: Value = match serde_json::from_str(&response_text) {
        Ok(json) => json,
        Err(_) => return Err("Error parsing JSON from iTaigi".to_string()),
    };

    let mut results = Vec::new();

    // Parse the 列表 array
    if let Some(list) = json.get("列表").and_then(|v| v.as_array()) {
        for item in list.iter().take(3) {
            // Limit to 3 results
            // Get 外語資料 (foreign word)
            let foreign_word = item
                .get("外語資料")
                .and_then(|v| v.as_str())
                .unwrap_or("N/A");

            // Get the first 新詞文本 entry if available
            if let Some(new_word_list) = item.get("新詞文本").and_then(|v| v.as_array()) {
                if let Some(first_entry) = new_word_list.first() {
                    let taigi_text = first_entry
                        .get("文本資料")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let pronunciation = first_entry
                        .get("音標資料")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let contributor = first_entry
                        .get("貢獻者")
                        .and_then(|v| v.as_str())
                        .unwrap_or("匿名");
                    let good_votes = first_entry
                        .get("按呢講好")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let bad_votes = first_entry
                        .get("按呢無好")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    // Create iTaigi URL
                    let itaigi_url = format!("https://itaigi.tw/k/{}", foreign_word);

                    results.push(format!(
                        "🏷️ {} → {} [{}] (👍{} 👎{}) by {} - {}",
                        foreign_word,
                        taigi_text,
                        pronunciation,
                        good_votes,
                        bad_votes,
                        contributor,
                        itaigi_url
                    ));
                }
            }
        }
    }

    // If no results from 列表, check 其他建議
    if results.is_empty() {
        if let Some(suggestions) = json.get("其他建議").and_then(|v| v.as_array()) {
            for suggestion in suggestions.iter().take(3) {
                let taigi_text = suggestion
                    .get("文本資料")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");
                let pronunciation = suggestion
                    .get("音標資料")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");

                // Get associated foreign words if available
                let mut foreign_words = Vec::new();
                if let Some(foreign_list) = suggestion
                    .get("按呢講的外語列表")
                    .and_then(|v| v.as_array())
                {
                    for foreign_item in foreign_list.iter().take(2) {
                        if let Some(foreign_word) =
                            foreign_item.get("外語資料").and_then(|v| v.as_str())
                        {
                            foreign_words.push(foreign_word);
                        }
                    }
                }

                let foreign_display = if foreign_words.is_empty() {
                    keyword.to_string()
                } else {
                    foreign_words.join(", ")
                };

                results.push(format!(
                    "🏷️ {} → {} [{}] (建議) - https://itaigi.tw",
                    foreign_display, taigi_text, pronunciation
                ));
            }
        }
    }

    Ok(results)
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from bots and messages not in the target channel
        if msg.author.bot || msg.channel_id.to_string() != "1372944023026794576".to_string() {
            return;
        }

        let keyword = msg.content.trim();

        if keyword.is_empty() {
            if let Err(why) = msg
                .channel_id
                .say(&ctx.http, "Please provide a keyword to search for.")
                .await
            {
                println!("Error sending empty keyword message: {why:?}");
            }
            return;
        }

        // Search all three sources concurrently
        let (taigitv_result, sutian_result, itaigi_result) = tokio::join!(
            search_taigitv(keyword),
            search_sutian(keyword),
            search_itaigi(keyword)
        );

        let mut all_results = Vec::new();
        let mut error_messages = Vec::new();

        // Collect TaigiTV results
        match taigitv_result {
            Ok(mut results) => all_results.append(&mut results),
            Err(err) => error_messages.push(format!("TaigiTV: {}", err)),
        }

        // Collect Sutian results
        match sutian_result {
            Ok(mut results) => all_results.append(&mut results),
            Err(err) => error_messages.push(format!("Sutian: {}", err)),
        }

        // Collect iTaigi results
        match itaigi_result {
            Ok(mut results) => all_results.append(&mut results),
            Err(err) => error_messages.push(format!("iTaigi: {}", err)),
        }

        // Handle results
        if !all_results.is_empty() {
            let count = all_results.len();
            let results_text = all_results.join("\n");
            let response_message = if count == 1 {
                format!("Found 1 result for \"{}\":\n{}", keyword, results_text)
            } else {
                format!(
                    "Found {} results for \"{}\":\n{}",
                    count, keyword, results_text
                )
            };

            // Add error info if some sources failed
            let final_message = if !error_messages.is_empty() {
                format!(
                    "{}\n\n⚠️ Some sources had issues: {}",
                    response_message,
                    error_messages.join(", ")
                )
            } else {
                response_message
            };

            if let Err(why) = msg.channel_id.say(&ctx.http, &final_message).await {
                println!("Error sending message: {why:?}");
            }
        } else if !error_messages.is_empty() {
            // All sources failed
            let error_msg = format!(
                "Could not search any sources. Errors: {}",
                error_messages.join(", ")
            );
            if let Err(why) = msg.channel_id.say(&ctx.http, &error_msg).await {
                println!("Error sending error message: {why:?}");
            }
        } else {
            // No results found
            if let Err(why) = msg.react(&ctx.http, '❌').await {
                println!("Error adding reaction: {why:?}");
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
