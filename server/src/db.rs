use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::{
    FromRow, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use uuid::Uuid;

pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

#[derive(Deserialize)]
struct SeedBar {
    name: String,
    address: Option<String>,
    city: Option<String>,
    country: Option<String>,
    lat: f64,
    lng: f64,
    /// True when the pin is known to be on the venue.
    #[serde(default)]
    verified: bool,
}

/// Loads the starter venues on first run. Pins not marked verified start out
/// unverified until someone drags them onto the real venue.
pub async fn seed_if_empty(pool: &SqlitePool) -> anyhow::Result<()> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM bars")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let seed: Vec<SeedBar> = serde_json::from_str(include_str!("../seed/bars.json"))?;
    for bar in seed {
        insert_bar(
            pool,
            &NewBar {
                name: bar.name,
                address: bar.address,
                city: bar.city,
                country: bar.country,
                lat: bar.lat,
                lng: bar.lng,
                player_offers: None,
                website: None,
            },
            bar.verified,
        )
        .await?;
    }
    tracing::info!("seeded starter bars");
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct NewBar {
    pub name: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub lat: f64,
    pub lng: f64,
    pub player_offers: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewRating {
    pub lighting: i64,
    pub table_space: i64,
    pub offers: i64,
    pub comment: Option<String>,
}

#[derive(Debug, FromRow)]
struct BarRow {
    id: String,
    name: String,
    address: Option<String>,
    city: Option<String>,
    country: Option<String>,
    lat: f64,
    lng: f64,
    location_verified: bool,
    player_offers: Option<String>,
    website: Option<String>,
    rating_count: i64,
    avg_lighting: Option<f64>,
    avg_table_space: Option<f64>,
    avg_offers: Option<f64>,
}

/// A bar as a GeoJSON feature, so the map can use the list as a source as-is.
#[derive(Clone, Debug, Serialize)]
pub struct BarFeature {
    #[serde(rename = "type")]
    kind: &'static str,
    pub id: String,
    geometry: Point,
    pub properties: BarProperties,
}

#[derive(Clone, Debug, Serialize)]
struct Point {
    #[serde(rename = "type")]
    kind: &'static str,
    coordinates: [f64; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct BarProperties {
    pub id: String,
    pub name: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub location_verified: bool,
    pub player_offers: Option<String>,
    pub website: Option<String>,
    pub rating_count: i64,
    pub avg_lighting: Option<f64>,
    pub avg_table_space: Option<f64>,
    pub avg_offers: Option<f64>,
    /// Mean of the three averages; drives the pin colour.
    pub score: Option<f64>,
}

impl From<BarRow> for BarFeature {
    fn from(row: BarRow) -> Self {
        let score = match (row.avg_lighting, row.avg_table_space, row.avg_offers) {
            (Some(l), Some(t), Some(o)) => Some((l + t + o) / 3.0),
            _ => None,
        };
        BarFeature {
            kind: "Feature",
            id: row.id.clone(),
            geometry: Point {
                kind: "Point",
                coordinates: [row.lng, row.lat],
            },
            properties: BarProperties {
                id: row.id,
                name: row.name,
                address: row.address,
                city: row.city,
                country: row.country,
                location_verified: row.location_verified,
                player_offers: row.player_offers,
                website: row.website,
                rating_count: row.rating_count,
                avg_lighting: row.avg_lighting,
                avg_table_space: row.avg_table_space,
                avg_offers: row.avg_offers,
                score,
            },
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct Rating {
    pub id: String,
    pub lighting: i64,
    pub table_space: i64,
    pub offers: i64,
    pub comment: Option<String>,
    pub created_at: String,
}

// A macro rather than a const so queries can be built with concat! and stay
// static strings.
macro_rules! bar_select {
    () => {
        "
    SELECT b.id, b.name, b.address, b.city, b.country, b.lat, b.lng,
           b.location_verified, b.player_offers, b.website,
           COUNT(r.id)        AS rating_count,
           AVG(r.lighting)    AS avg_lighting,
           AVG(r.table_space) AS avg_table_space,
           AVG(r.offers)      AS avg_offers
    FROM bars b
    LEFT JOIN ratings r ON r.bar_id = b.id"
    };
}

pub async fn list_bars(pool: &SqlitePool) -> sqlx::Result<Vec<BarFeature>> {
    let rows: Vec<BarRow> =
        sqlx::query_as(concat!(bar_select!(), " GROUP BY b.id ORDER BY b.name"))
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_bar(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<BarFeature>> {
    let row: Option<BarRow> =
        sqlx::query_as(concat!(bar_select!(), " WHERE b.id = ? GROUP BY b.id"))
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(Into::into))
}

pub async fn insert_bar(
    pool: &SqlitePool,
    bar: &NewBar,
    location_verified: bool,
) -> sqlx::Result<String> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO bars (id, name, address, city, country, lat, lng, location_verified, player_offers, website)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&bar.name)
    .bind(&bar.address)
    .bind(&bar.city)
    .bind(&bar.country)
    .bind(bar.lat)
    .bind(bar.lng)
    .bind(location_verified)
    .bind(&bar.player_offers)
    .bind(&bar.website)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Moves a pin. Returns false if the bar doesn't exist.
pub async fn move_bar(pool: &SqlitePool, id: &str, lat: f64, lng: f64) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE bars SET lat = ?, lng = ?, location_verified = 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?",
    )
    .bind(lat)
    .bind(lng)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn insert_rating(
    pool: &SqlitePool,
    bar_id: &str,
    rating: &NewRating,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO ratings (id, bar_id, lighting, table_space, offers, comment)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(bar_id)
    .bind(rating.lighting)
    .bind(rating.table_space)
    .bind(rating.offers)
    .bind(&rating.comment)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn recent_ratings(pool: &SqlitePool, bar_id: &str) -> sqlx::Result<Vec<Rating>> {
    sqlx::query_as(
        "SELECT id, lighting, table_space, offers, comment, created_at
         FROM ratings WHERE bar_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 20",
    )
    .bind(bar_id)
    .fetch_all(pool)
    .await
}
