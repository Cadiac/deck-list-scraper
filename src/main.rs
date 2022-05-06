use rusqlite::{Connection, Result};
use deck_list_scraper::{mtgo, tcdecks, db};

fn main() -> Result<()> {
    let conn = Connection::open("decklists.db")?;

    db::setup(&conn)?;
    tcdecks::scrape(&conn)?;
    // mtgo::scrape(&conn)?;

    Ok(())
}
