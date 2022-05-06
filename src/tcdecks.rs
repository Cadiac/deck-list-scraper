use chrono::prelude::NaiveDate;
use reqwest::blocking::Client;
use reqwest::Url;
use rusqlite::{Connection, Result};
use select::document::Document;
use select::node::Node;
use select::predicate::{Class, Name};
use std::time::Duration;
use std::{fmt, thread, time};

use crate::db;
use crate::deck::{Decklist, Format};

const BASE_URL: &str = "https://www.tcdecks.net";
const DECKLISTS_ENDPOINT: &str = "/format.php";
const FORMATS: &[(&str, Format)] = &[
    ("Vintage", Format::Vintage),
    ("Vintage Old School", Format::OldSchool),
    ("Premodern", Format::Premodern),
    ("Legacy", Format::Legacy),
    ("Modern", Format::Modern),
    ("Pauper", Format::Pauper),
];

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
        .take(1) // TODO
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
                // for decklist in decklists.into_iter() {
                //     if let Err(e) = db::insert_decklist(conn, &decklist) {
                //         eprintln!("Failed to insert decklist: {}", e);
                //     }
                // }

                // db::insert_scraped_link(conn, &link, true, None)?;
            }
            Err(e) => {
                eprintln!("Failed to scrape decklists: {}", e);
                // db::insert_scraped_link(conn, &link, false, Some(&e.to_string()))?;
            }
        }

        // Lets be polite
        thread::sleep(time::Duration::from_millis(1000));
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

        println!("[{format_param}/{page}] Scanning event links, total {}", links.len());

        let res_html = client.get(url).send()?.text()?;
        let page_links = parse_tourney_links(res_html, *format);

        if page_links.is_empty() || page > 1 {
            return Ok(links);
        }

        links.extend(page_links);
        page += 1;

        thread::sleep(time::Duration::from_millis(1000));
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

    println!("Found {} deck links", deck_links.len());

    for (index, (format, deck_link)) in deck_links.iter().enumerate() {
        println!("[Event {event_link}: {}/{}] {}: {}", index + 1, deck_links.len(), format, deck_link);

        let deck_url = Url::parse(BASE_URL)?.join(deck_link)?;
        let res_html = client.get(deck_url).send()?.text()?;

        let document = Document::from(res_html.as_str());

        let table = document.find(Name("table")).next().unwrap();
        let mut rows = table.find(Name("tr"));

        let name = rows.next().unwrap().find(Name("th")).next().unwrap();

        println!("Player name: {}", name.text());

        thread::sleep(time::Duration::from_millis(1000));
    }

    // let decklist = parse_decklist(client, link)?;

    // if let Err(e) = db::insert_decklist(&decklist) {
    //     eprintln!("Failed to insert decklist: {}", e);
    // }

    // let document = Document::from(res.as_str());

    // let date = document
    //     .find(Class("posted-in"))
    //     .next()
    //     .unwrap()
    //     .children()
    //     .nth(2)
    //     .and_then(|node| {
    //         node.text()
    //             .trim()
    //             .strip_prefix("on ")
    //             .and_then(|date_str| NaiveDate::parse_from_str(date_str, "%B %d, %Y").ok())
    //     });

    // let decklist_containers = document.find(Class("deck-group"));

    // let decklists = decklist_containers
    //     .map(|container| {
    //         let mainboard = container
    //             .find(Class("sorted-by-overview-container"))
    //             .next()
    //             .unwrap()
    //             .find(Class("row"))
    //             .flat_map(|row| parse_card_row(&row))
    //             .collect();

    //         let sideboard = container
    //             .find(Class("sorted-by-sideboard-container"))
    //             .next()
    //             .map_or_else(
    //                 || Vec::new(),
    //                 |node| {
    //                     node.find(Class("row"))
    //                         .flat_map(|row| parse_card_row(&row))
    //                         .collect()
    //                 },
    //             );

    //         let player = container
    //             .find(Class("deck-meta"))
    //             .next()
    //             .unwrap()
    //             .find(Name("h4"))
    //             .next()
    //             .map(|node| node.text().trim().to_owned());

    //         let event = container
    //             .find(Class("deck-meta"))
    //             .next()
    //             .unwrap()
    //             .find(Name("h5"))
    //             .next()
    //             .map(|node| node.text().trim().to_owned());

    //         Decklist {
    //             event,
    //             player,
    //             format: *format,
    //             date,
    //             mainboard,
    //             sideboard,
    //         }
    //     })
    //     .collect();

    Ok(vec![])
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
