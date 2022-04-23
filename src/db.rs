use rusqlite::{params, named_params, Connection, Result};

use crate::deck::Decklist;

pub fn setup(conn: &Connection) -> Result<()> {
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

    Ok(())
}

pub fn insert_decklist(conn: &Connection, decklist: &Decklist) -> Result<()> {
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

    for (count, card) in decklist.mainboard.iter() {                    
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

    for (count, card) in decklist.sideboard.iter() {
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

    Ok(())
}