use chrono::prelude::*;
use reqwest::blocking::Client;
use reqwest::Url;
use rusqlite::{params, named_params, Connection, Result};
use select::document::Document;
use select::node::Node;
use select::predicate::{Class, Name};
use serde::Deserialize;

use std::{fmt, thread, time};

const BASE_URL: &str = "https://magic.wizards.com";
const DECKLISTS_ENDPOINT: &str = "/en/section-articles-see-more-ajax?dateoff=&l=en&f=9041&search-result-theme=&limit=10&fromDate=&toDate=&sort=DESC&word=&offset=0";

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Format {
    Standard,
    Pioneer,
    Modern,
    Legacy,
    Vintage,
    Pauper,
    Historic,
    Alchemy,
    Unknown,
}

impl From<&str> for Format {
    fn from(i: &str) -> Self {
        match i {
            "standard" => Format::Standard,
            "pioneer" => Format::Pioneer,
            "modern" => Format::Modern,
            "legacy" => Format::Legacy,
            "vintage" => Format::Vintage,
            "pauper" => Format::Pauper,
            "historic" => Format::Historic,
            "alchemy" => Format::Alchemy,
            _ => Format::Unknown,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Format::Standard => write!(f, "standard"),
            Format::Pioneer => write!(f, "pioneer"),
            Format::Modern => write!(f, "modern"),
            Format::Legacy => write!(f, "legacy"),
            Format::Vintage => write!(f, "vintage"),
            Format::Pauper => write!(f, "pauper"),
            Format::Historic => write!(f, "historic"),
            Format::Alchemy => write!(f, "alchemy"),
            Format::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone)]
struct NotFoundError;

impl fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "article is not found")
    }
}
impl std::error::Error for NotFoundError {}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DecklistLinks {
    data: Vec<String>,
    _status: i32,
    _offset: i32,
    _display_see_more: i32,
}

#[derive(Debug)]
struct Decklist {
    format: Format,
    player: Option<String>,
    event: Option<String>,
    date: Option<NaiveDate>,
    mainboard: Vec<(usize, String)>,
    sideboard: Vec<(usize, String)>,
}

fn main() -> Result<()> {
    let client = Client::new();

    let conn = Connection::open("decklists.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS decks (
                id INTEGER PRIMARY KEY,
                format TEXT NOT NULL,
                event TEXT,
                date TEXT,
                player TEXT
            )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS deck_cards (
                deck_id INTEGER,
                card_id INTEGER,
                count INTEGER,
                is_sideboard BOOLEAN,
                FOREIGN KEY(deck_id) REFERENCES decks(id),
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
        [],
    )?;

    let links = find_latest_decklists(&client).unwrap();

    for (format, link) in links {
        if let Ok(decklists) = scrape_decklists(&client, &link, format) {
            for decklist in decklists.into_iter() {
                conn.execute(
                    "INSERT INTO decks (format, event, date, player) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        decklist.format.to_string(),
                        decklist.event,
                        decklist.date.and_then(|d| Some(d.to_string())),
                        decklist.player,
                    ],
                )?;

                let deck_id = conn.last_insert_rowid();
                let mut cards_query = conn.prepare("SELECT id FROM cards WHERE name = :name;")?;

                for (count, card) in decklist.mainboard.into_iter() {                    
                    let card_id = 
                        if let Some(row) = cards_query.query(named_params!{ ":name": card })?.next()? {
                            row.get(0)?
                        } else {
                            conn.execute(
                                "INSERT INTO cards (name) VALUES (?1)",
                                params![card],
                            )?;
                            conn.last_insert_rowid()
                        };

                    conn.execute(
                        "INSERT INTO deck_cards (deck_id, card_id, count, is_sideboard) VALUES (?1, ?2, ?3, 0)",
                        params![
                            deck_id,
                            card_id,
                            count
                        ],
                    )?;
                }

                for (count, card) in decklist.sideboard.into_iter() {
                    let card_id = 
                        if let Some(row) = cards_query.query(named_params!{ ":name": card })?.next()? {
                            row.get(0)?
                        } else {
                            conn.execute(
                                "INSERT INTO cards (name) VALUES (?1)",
                                params![card],
                            )?;
                            conn.last_insert_rowid()
                        };

                    conn.execute(
                        "INSERT INTO deck_cards (deck_id, card_id, count, is_sideboard) VALUES (?1, ?2, ?3, 1)",
                        params![
                            deck_id,
                            card_id,
                            count
                        ],
                    )?;
                }
            }
        }

        // Lets be polite
        thread::sleep(time::Duration::from_millis(1000));
    }

    Ok(())
}

fn find_latest_decklists(
    client: &Client,
) -> Result<Vec<(Format, String)>, Box<dyn std::error::Error>> {
    let url = Url::parse(BASE_URL)?.join(DECKLISTS_ENDPOINT)?;
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
    format: Format,
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
                .unwrap()
                .find(Class("row"))
                .flat_map(|row| parse_card_row(&row))
                .collect();

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
                format,
                date,
                mainboard,
                sideboard,
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
