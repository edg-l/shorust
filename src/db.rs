use rand::distributions::Alphanumeric;
use rand::Rng;
use rusqlite::OptionalExtension;
use rusqlite::NO_PARAMS;

pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;


pub async fn create_table(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "
        create table if not exists urls (
            id text primary key,
            url text not null unique,
            hits bigint default 0
        )
        ",
        NO_PARAMS,
    )
}

fn generate_id() -> String {
    let mut rng = rand::thread_rng();
    (&mut rng)
        .sample_iter(Alphanumeric)
        .take(6)
        .map(char::from)
        .collect()
}

pub async fn get_url_by_id(conn: &Connection, id: &str) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("select url from urls where id = ? limit 1")?;
    stmt.query_row(&[id], |r| r.get(0)).optional()
}

pub async fn get_id_by_url(conn: &Connection, url: &str) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("select id from urls where url = ? limit 1")?;
    stmt.query_row(&[url], |r| r.get(0)).optional()
}

pub async fn add_url(conn: &Connection, url: &str) -> Result<String, rusqlite::Error> {
    let mut stmt = conn.prepare("insert into urls (id, url) values (?, ?)")?;

    let id = generate_id();

    stmt.execute(&[id.clone(), url.to_string()])?;

    Ok(id)
}

pub async fn add_url_hit(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("update urls set hits = hits + 1 where id = ?")?;
    stmt.execute(&[id.clone()])?;
    Ok(())
}

