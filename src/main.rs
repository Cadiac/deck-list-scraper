use rusqlite::{Connection, Result};

use std::fmt;
use deck_list_scraper::{mtgo, db};

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

fn main() -> Result<()> {
    let conn = Connection::open("decklists.db")?;

    db::setup(&conn)?;
    mtgo::scrape(&conn)?;

    Ok(())
}
