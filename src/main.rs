use chrono::prelude::*;
use reqwest::blocking::Client;
use reqwest::Url;
use select::document::Document;
use select::node::Node;
use select::predicate::{Class, Name};
use serde::Deserialize;
use std::{thread, time};

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
    event: Option<String>,
    player: Option<String>,
    format: Format,
    date: Option<NaiveDate>,
    mainboard: Vec<(usize, String)>,
    sideboard: Vec<(usize, String)>,
}

fn main() {
    let client = Client::new();

    let links = find_latest_decklists(&client).unwrap();

    for (format, link) in links {
        let decklists = scrape_decklists(&client, &link, format);

        println!("Got decklists {:?}", decklists);

        // Lets be polite
        thread::sleep(time::Duration::from_millis(10000));
    }
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
                .map(|row| parse_card_row(&row))
                .collect();

            let sideboard = container
                .find(Class("sorted-by-sideboard-container"))
                .next()
                .unwrap()
                .find(Class("row"))
                .map(|row| parse_card_row(&row))
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

fn parse_card_row(card_row: &Node) -> (usize, String) {
    let count_str: String = card_row.find(Class("card-count")).next().unwrap().text();
    let count = count_str.parse::<usize>().unwrap();

    let name: String = card_row
        .find(Class("card-name"))
        .next()
        .unwrap()
        .find(Name("a"))
        .next()
        .unwrap()
        .text();
    (count, name)
}
