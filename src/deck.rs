use chrono::prelude::*;
use serde::Deserialize;

use std::fmt;

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
    Explorer,
    Premodern,
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
            "explorer" => Format::Explorer,
            "premodern" => Format::Premodern,
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
            Format::Explorer => write!(f, "explorer"),
            Format::Premodern => write!(f, "premodern"),
            Format::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DecklistLinks {
    pub data: Vec<String>,
    _status: i32,
    _offset: i32,
    _display_see_more: i32,
}

#[derive(Debug)]
pub struct Decklist {
    pub format: Format,
    pub player: Option<String>,
    pub event: Option<String>,
    pub date: Option<NaiveDate>,
    pub mainboard: Vec<(usize, String)>,
    pub sideboard: Vec<(usize, String)>,
}

#[derive(Debug)]
pub struct ScrapedLink {
    pub id: i32,
    pub link: String,
    pub is_success: bool,
    pub error_msg: Option<String>,
    pub created_at: String,
}