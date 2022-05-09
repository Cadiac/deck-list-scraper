use rusqlite::OptionalExtension;
use rusqlite::{named_params, params, Connection, Result};

use crate::deck::{Decklist, ScrapedLink};

pub fn setup(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS decks (
                id INTEGER PRIMARY KEY,
                name TEXT,
                format TEXT NOT NULL,
                event TEXT,
                date TEXT,
                player TEXT,
                archetype TEXT,
                result TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS scraped_links (
                id INTEGER PRIMARY KEY,
                link TEXT NOT NULL UNIQUE,
                is_success BOOLEAN,
                error_msg TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
        [],
    )?;

    Ok(())
}

pub fn insert_decklist(conn: &Connection, decklist: &Decklist) -> Result<()> {
    conn.execute(
        "INSERT INTO decks (format, event, date, player, archetype, result, name) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            decklist.format.to_string(),
            decklist.event,
            decklist.date.and_then(|d| Some(d.to_string())),
            decklist.player,
            decklist.archetype,
            decklist.result,
            decklist.name,
        ],
    )?;

    let deck_id = conn.last_insert_rowid();
    let mut cards_query = conn.prepare("SELECT id FROM cards WHERE name = :name;")?;

    for (count, card) in decklist.mainboard.iter() {
        let card_id =
            if let Some(row) = cards_query.query(named_params! { ":name": card })?.next()? {
                row.get(0)?
            } else {
                conn.execute("INSERT INTO cards (name) VALUES (?1)", params![card])?;
                conn.last_insert_rowid()
            };

        conn.execute(
            "INSERT INTO deck_cards (deck_id, card_id, count, is_sideboard) VALUES (?1, ?2, ?3, 0)",
            params![deck_id, card_id, count],
        )?;
    }

    for (count, card) in decklist.sideboard.iter() {
        let card_id =
            if let Some(row) = cards_query.query(named_params! { ":name": card })?.next()? {
                row.get(0)?
            } else {
                conn.execute("INSERT INTO cards (name) VALUES (?1)", params![card])?;
                conn.last_insert_rowid()
            };

        conn.execute(
            "INSERT INTO deck_cards (deck_id, card_id, count, is_sideboard) VALUES (?1, ?2, ?3, 1)",
            params![deck_id, card_id, count],
        )?;
    }

    Ok(())
}

pub fn insert_scraped_link(
    conn: &Connection,
    link: &str,
    is_success: bool,
    error_msg: Option<&str>,
) -> Result<usize> {
    match error_msg {
        Some(msg) => conn.execute(
            "INSERT INTO scraped_links (link, is_success, error_msg) VALUES (?1, ?2, ?3)",
            params![link, is_success, msg],
        ),
        None => conn.execute(
            "INSERT INTO scraped_links (link, is_success) VALUES (?1, ?2)",
            params![link, is_success],
        ),
    }
}

pub fn find_scraped_link(conn: &Connection, link: &str) -> Result<Option<ScrapedLink>> {
    let mut stmt = conn.prepare(
        "SELECT id, link, is_success, error_msg, created_at
            FROM scraped_links
            WHERE link = ?1",
    )?;
    stmt.query_row([link], |row| {
        Ok(ScrapedLink {
            id: row.get(0)?,
            link: row.get(1)?,
            is_success: row.get(2)?,
            error_msg: row.get(3)?,
            created_at: row.get(4)?,
        })
    })
    .optional()
}
