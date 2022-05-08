use chrono::prelude::NaiveDate;
use reqwest::blocking::Client;
use reqwest::Url;
use rusqlite::{Connection, Result};
use select::document::Document;
use select::node::Children;
use select::predicate::{Class, Name};
use std::time::Duration;
use std::{fmt, thread, time};

use crate::db;
use crate::deck::{Decklist, Format};

const BASE_URL: &str = "https://www.tcdecks.net";
const DECKLISTS_ENDPOINT: &str = "/format.php";
const FORMATS: &[(&str, Format)] = &[
    ("Premodern", Format::Premodern),
    ("Vintage", Format::Vintage),
    ("Vintage Old School", Format::OldSchool),
    ("Legacy", Format::Legacy),
    ("Modern", Format::Modern),
    ("Pauper", Format::Pauper),
];
const SLEEP_DELAY: u64 = 1000;

#[derive(Debug, Clone)]
struct NotFoundError;

impl fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "decklists not found")
    }
}
impl std::error::Error for NotFoundError {}

pub fn scrape(conn: &Connection) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    let links: Vec<(Format, String)> = FORMATS
        .iter()
        .flat_map(|format| match find_latest_decklists(&client, format) {
            Ok(links) => links,
            Err(e) => {
                eprintln!("Failed to find decklists for {}: {}", format.0, e);
                vec![]
            }
        })
        .collect();

    println!("Found {} links", links.len());

    for (index, (format, link)) in links.iter().enumerate() {
        println!("[{}/{}] {}: {}", index + 1, links.len(), format, link);

        match db::find_scraped_link(conn, link)? {
            Some(scraped) => {
                if scraped.is_success {
                    println!(
                        "[{}/{}] Already successfully scraped, skipping",
                        index + 1,
                        links.len()
                    );
                    continue;
                }
            }
            None => {}
        }

        match scrape_decklists(&client, &link, format) {
            Ok(decklists) => {
                for decklist in decklists.into_iter() {
                    if let Err(e) = db::insert_decklist(conn, &decklist) {
                        eprintln!("Failed to insert decklist: {}", e);
                    }
                }

                db::insert_scraped_link(conn, &link, true, None)?;
            }
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

fn find_latest_decklists(
    client: &Client,
    (format_param, format): &(&str, Format),
) -> Result<Vec<(Format, String)>, Box<dyn std::error::Error>> {
    let mut page = 1;
    let mut links = Vec::new();

    loop {
        let url = Url::parse(BASE_URL)?
            .join(format!("{DECKLISTS_ENDPOINT}?format={format_param}&page={page}").as_str())?;

        println!(
            "[{format_param}/{page}] Scanning event links, total {}",
            links.len()
        );

        let res_html = client.get(url).send()?.text()?;
        let page_links = parse_tourney_links(res_html, *format);

        if page_links.is_empty() || page > 1 {
            return Ok(links);
        }

        links.extend(page_links);
        page += 1;

        thread::sleep(time::Duration::from_millis(SLEEP_DELAY));
    }
}

fn parse_tourney_links(res_html: String, format: Format) -> Vec<(Format, String)> {
    Document::from(res_html.as_str())
        .find(Class("tourney_list"))
        .next()
        .unwrap()
        .find(Class("principal"))
        .filter_map(|node| {
            node.find(Name("a")).next().map(|link_node| {
                let href = link_node.attr("href")?;
                if href.starts_with("deck.php") {
                    Some((format, href.to_owned()))
                } else {
                    None
                }
            })
        })
        .flatten()
        .collect()
}

fn scrape_decklists(
    client: &Client,
    event_link: &str,
    format: &Format,
) -> Result<Vec<Decklist>, Box<dyn std::error::Error>> {
    let url = Url::parse(BASE_URL)?.join(event_link)?;
    let res_html = client.get(url).send()?.text()?;

    let deck_links = parse_tourney_links(res_html, *format);

    let mut decklists = Vec::new();

    for (index, (format, deck_link)) in deck_links.iter().enumerate() {
        println!(
            "[Event {event_link}, deck: {}/{}] {}: {}",
            index + 1,
            deck_links.len(),
            format,
            deck_link
        );

        let deck_url = Url::parse(BASE_URL)?.join(deck_link)?;
        let res_html = client.get(deck_url).send()?.text()?;

        let document = Document::from(res_html.as_str());

        let mut legend = document.find(Name("legend")).next().unwrap().children();

        // Skip the empty whitespaces
        let event = legend.nth(1).map(|node| node.text());
        let date = legend.nth(1).and_then(|node| {
            let date_text = node.text();
            date_text.trim().split(" | ").nth(2).and_then(|date_str| {
                NaiveDate::parse_from_str(date_str.strip_prefix("Date: ").unwrap_or(""), "%d/%m/%Y")
                    .ok()
            })
        });

        let table = document.find(Name("table")).next().unwrap();
        let mut rows = table.find(Name("tr"));

        let mut header_row = rows.next().ok_or("no table rows")?.find(Name("th"));

        let name_header = header_row
            .next()
            .map(|node| node.text())
            .ok_or("no name header")?;

        let mut name_and_archetype = name_header.split(" playing ");
        let player_name = name_and_archetype.next().ok_or("no name")?.trim();
        let archetype = name_and_archetype.next().ok_or("no archetype")?.trim();

        let position_th = header_row
            .next()
            .map(|node| node.text())
            .ok_or("no position header")?;

        let position = position_th
            .trim()
            .strip_prefix("Position: ")
            .ok_or("no position")?;

        let deck_name_header = rows
            .next()
            .ok_or("no table rows")?
            .find(Name("th"))
            .next()
            .map(|node| node.text())
            .ok_or("no deck name header")?;

        let deck_name = deck_name_header
            .trim()
            .strip_prefix("Deck Name: ")
            .ok_or("no deck name")?;

        let mut cards_row = rows.next().ok_or("no table rows")?.find(Name("td"));

        let mut mainboard = parse_cards(cards_row
            .next()
            .ok_or("no cards")?
            .children());

        let mut mainboard_2 = parse_cards(cards_row
            .next()
            .ok_or("no cards")?
            .children());

        mainboard.append(&mut mainboard_2);

        let sideboard = parse_cards(cards_row
            .next()
            .ok_or("no cards")?
            .children());

        let decklist = Decklist {
            event,
            player: Some(player_name.to_owned()),
            format: *format,
            date,
            mainboard,
            sideboard,
            archetype: Some(archetype.to_owned()),
            result: Some(position.to_owned()),
            name: Some(deck_name.to_owned()),
        };

        decklists.push(decklist);

        thread::sleep(time::Duration::from_millis(SLEEP_DELAY));
    }

    Ok(decklists)
}

fn parse_cards(card_rows: Children) -> Vec<(usize, String)> {
    let mut amount = 1;

    card_rows
        .flat_map(|node| match node.name() {
            Some("h6") => None,
            Some("a") => Some((amount, node.text())),
            _ => {
                let text = node.text();
                amount = text.trim().parse::<usize>().unwrap_or(1);
                None
            }
        })
        .collect::<Vec<_>>()
}
