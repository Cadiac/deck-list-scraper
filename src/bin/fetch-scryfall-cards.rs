use rusqlite::{Connection, Result};
use deck_list_scraper::{db};

fn main() -> Result<()> {
    let conn = Connection::open("decklists.db")?;

    db::setup(&conn)?;

    let mut updated: usize = 0;

    println!("Fetching cards...");
    match scryfall::bulk::oracle_cards() {
        Ok(cards) => {
            println!("Received cards, starting to update database");

            for api_card in cards {
                updated += 1;

                if updated % 1000 == 0 {
                    println!("Updated {} cards...", updated);
                }

                if let Ok(card) = api_card {
                    match db::upsert_card(&conn, &card) {
                        Ok(_) => {},
                        Err(e) => eprintln!("Failed to upsert card: {}", e),
                    }
                }

            }
        },
        Err(e) => {
            println!("Failed to fetch cards: {}", e);
        }
    }

    Ok(())
}
