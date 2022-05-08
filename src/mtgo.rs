use reqwest::blocking::Client;
use reqwest::Url;
use select::document::Document;
use select::node::Node;
use select::predicate::{Class, Name};
use std::{fmt, thread, time};
use std::time::Duration;
use chrono::prelude::{NaiveDate};
use rusqlite::{Connection, Result};

use crate::deck::{Format, Decklist, DecklistLinks};
use crate::db;

const BASE_URL: &str = "https://magic.wizards.com";
const DECKLISTS_ENDPOINT: &str = "/en/section-articles-see-more-ajax?dateoff=&l=en&f=9041&search-result-theme=&fromDate=&toDate=&sort=DESC&word=";
const SLEEP_DELAY: u64 = 1000;

#[derive(Debug, Clone)]
struct NotFoundError;

impl fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "article is not found")
    }
}
impl std::error::Error for NotFoundError {}

pub fn scrape(conn: &Connection) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
        
    let links = find_latest_decklists(&client).unwrap();

    println!("Found {} links", links.len());

    for (index, (format, link)) in links.iter().enumerate() {
        println!("[{}/{}] {}: {}", index + 1, links.len(), format, link);

        match db::find_scraped_link(conn, link)? {
            Some(scraped) => {
                if scraped.is_success {
                    println!("[{}/{}] Already successfully scraped, skipping", index + 1, links.len());
                    continue;
                }
            },
            None => {},
        }

        match scrape_decklists(&client, &link, format) {
            Ok(decklists) => {
                for decklist in decklists.into_iter() {
                    if let Err(e) = db::insert_decklist(conn, &decklist) {
                        eprintln!("Failed to insert decklist: {}", e);
                    }
                }

                db::insert_scraped_link(conn, &link, true, None)?;
            },
            Err(e) => {
                eprintln!("Failed to scrape decklists: {}", e);
                db::insert_scraped_link(conn, &link, false, Some(&e.to_string()))?;
            }
        }

        // Lets be polite
        thread::sleep(time::Duration::from_millis(SLEEP_DELAY));
    }

    Ok(())
}

fn find_latest_decklists(client: &Client) -> Result<Vec<(Format, String)>, Box<dyn std::error::Error>> {
    let offset = 0;
    let limit = 20;
    let url = Url::parse(BASE_URL)?.join(format!("{DECKLISTS_ENDPOINT}&offset={offset}&limit={limit}").as_str())?;
    let res = client.get(url).send()?.text()?;

    let parsed: DecklistLinks = serde_json::from_str(&res)?;

    let links = parsed
        .data
        .into_iter()
        .map(|html_link| {
            let document = Document::from(html_link.as_str());
            let link_container = document
                .find(Class("article-item-extended"))
                .next()
                .unwrap();

            let link = link_container
                .find(Name("a"))
                .filter_map(|n| n.attr("href"))
                .map(|s| s.to_string())
                .collect();

            let title_container = document.find(Class("title")).next().unwrap();

            let format = title_container
                .find(Name("h3"))
                .next()
                .unwrap()
                .text()
                .to_lowercase()
                .split(' ')
                .next()
                .unwrap()
                .into();

            (format, link)
        })
        .collect();

    Ok(links)
}

fn scrape_decklists(
    client: &Client,
    link: &str,
    format: &Format,
) -> Result<Vec<Decklist>, Box<dyn std::error::Error>> {
    let url = Url::parse(BASE_URL)?.join(link)?;
    let res = client.get(url).send()?.text()?;

    if res.contains("no result found") {
        return Err(Box::new(NotFoundError));
    }

    let document = Document::from(res.as_str());

    let date = document
        .find(Class("posted-in"))
        .next()
        .unwrap()
        .children()
        .nth(2)
        .and_then(|node| {
            node.text()
                .trim()
                .strip_prefix("on ")
                .and_then(|date_str| NaiveDate::parse_from_str(date_str, "%B %d, %Y").ok())
        });

    let decklist_containers = document.find(Class("deck-group"));

    let decklists = decklist_containers
        .map(|container| {
            let mainboard = container
                .find(Class("sorted-by-overview-container"))
                .next()
                .unwrap()
                .find(Class("row"))
                .flat_map(|row| parse_card_row(&row))
                .collect();

            let sideboard = container
                .find(Class("sorted-by-sideboard-container"))
                .next()
                .map_or_else(|| Vec::new(), |node| {
                    node.find(Class("row"))
                        .flat_map(|row| parse_card_row(&row))
                        .collect()
                });

            let player = container
                .find(Class("deck-meta"))
                .next()
                .unwrap()
                .find(Name("h4"))
                .next()
                .map(|node| node.text().trim().to_owned());

            let event = container
                .find(Class("deck-meta"))
                .next()
                .unwrap()
                .find(Name("h5"))
                .next()
                .map(|node| node.text().trim().to_owned());

            Decklist {
                event,
                player,
                format: *format,
                date,
                mainboard,
                sideboard,
                archetype: None,
                result: Some("5-0".to_owned()),
                name: None,
            }
        })
        .collect();

    Ok(decklists)
}

fn parse_card_row(card_row: &Node) -> Option<(usize, String)> {
    let count_str: String = card_row.find(Class("card-count")).next()?.text();
    let count = count_str.parse::<usize>().ok()?;

    card_row
        .find(Class("card-name"))
        .next()
        .and_then(|node| {
            node.find(Name("a"))
                .next()
                .or_else(|| node.children().next())
        })
        .map(|node| (count, node.text()))
}
