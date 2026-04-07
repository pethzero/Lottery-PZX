use axum::{
    extract::{Query, State},
    http::{header::{CONTENT_TYPE, COOKIE}, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener; // ใช้ TcpListener แทน hyper::Server
use tokio_postgres::NoTls;
use tower_http::cors::{Any, CorsLayer}; // เพิ่มตัวนี้เพื่อแก้ปัญหา Any และ CorsLayer
use dotenvy::dotenv;
use serde_json::Value;
use uuid::Uuid;
use chrono::NaiveDate;
use tracing::{error, info};


const GLO_COOKIE: &str = "TS01402785=019c1ab27133f83f55703a15b40aadca49c20a84fea04373103d2aa6e7cdceed9a8d3693e2b86703b62ba85a2aa0018c5404f4ec7da93ba2acbf915234981f9fb07417d178; TS0195bd77=019c1ab27182255877f192a8b328605a5b9bc2412cbdc001b36076994e57215337d25c06eb54a25c615cd05fd914fd6aa0dc6ca16022dd85b503a5ac91f8681fef4b7886840e52e50f50a4e0bd6b59712c3c738f53e7a0af8f4e14fcbd17bd6b4cd77cb329; inb-session=2114897930.47873.0000; mba-session=rd2o00000000000000000000ffff0a121551o80";

#[derive(Serialize)]
struct ApiResponse {
    message: String,
    source: serde_json::Value,
}

#[derive(Deserialize)]
struct HttpBinQuery {
    name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // ส่วนของ CORS ที่คุณ Error ก่อนหน้านี้
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    dotenv().ok();

    let db_host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let db_port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let db_user = env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
    let db_pass = env::var("DB_PASS").unwrap_or_else(|_| "123456".to_string());
    let db_name = env::var("DB_NAME").unwrap_or_else(|_| "postgres".to_string());

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        db_user, db_pass, db_host, db_port, db_name
    );

    let pool = PgPool::connect(&database_url)
        .await
        .expect("failed to connect to postgres");

    let app = Router::new()
        .route("/api/httpbin", get(httpbin_handler))
        .route("/api/hello", get(hello_handler))
        .route("/api/last-lottery", get(last_lottery_handler))
        .route("/api/check_postgres", get(check_postgres_handler))
        .route("/api/import-lottery", post(import_lottery_handler))
        .with_state(pool)
        .layer(cors); // ใส่ CORS layer เข้าไป

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Rust API running at http://{}", addr);

    // แก้ไขตรงนี้: Axum 0.7 ใช้ TcpListener
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// --- Handler functions ด้านล่างนี้เหมือนเดิม หรือปรับปรุงตามความเหมาะสม ---

async fn hello_handler() -> Json<ApiResponse> {
    println!("Hello from handler");
    dbg!("DEBUG: hello_handler called");
    info!("เรียก hello_handler แล้ว");
    error!("มี error เกิดขึ้น");
    Json(ApiResponse {
        message: "Hello from Rust backend".to_string(),
        source: serde_json::json!({}),
    })
}

async fn last_lottery_handler() -> Result<Json<ApiResponse>, StatusCode> {
    let client = reqwest::Client::new();
    
    let resp = client
        .post("https://www.glo.or.th/api/lottery/getLatestLottery")
        .header(CONTENT_TYPE, "application/json")
        .header(COOKIE, GLO_COOKIE) // ใช้ Header Name จาก reqwest
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let body: serde_json::Value = resp.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(ApiResponse {
        message: "Latest lottery result from GLO".to_string(),
        source: body,
    }))
}


pub async fn import_lottery_handler(
    State(pool): State<PgPool>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // info!("import_lottery_handler received payload: {:?}", payload);

    // Validate top-level structure
    let response = payload
        .get("response")
        .and_then(|r| r.as_object())
        .ok_or_else(|| {
            error!("Missing or invalid 'response' field in payload");
            StatusCode::BAD_REQUEST
        })?;
    // info!("parsed response object: {:?}", response);

    // Parse and validate draw_id (UUID)
    let sheet_id_str = response
        .get("sheetId")
        .and_then(|s| s.as_str())
        .ok_or_else(|| {
            error!("Missing or invalid 'sheetId' field in response");
            StatusCode::BAD_REQUEST
        })?;
    // info!("parsed sheetId string: {}", sheet_id_str);

    let draw_id = Uuid::parse_str(sheet_id_str).map_err(|err| {
        error!("Invalid UUID in sheetId '{}': {}", sheet_id_str, err);
        StatusCode::BAD_REQUEST
    })?;


    // Parse and validate draw_date
    let draw_date_str = response
        .get("date")
        .and_then(|d| d.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let draw_date = NaiveDate::parse_from_str(draw_date_str, "%Y-%m-%d")
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    info!("Starting import for draw_date: {}, draw_id: {}", draw_date, draw_id);

    // // Start transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|err| {
            error!("Failed to start transaction: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // // Check for existing draw by date
    let existing_draw: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM lottery_draw WHERE draw_date = $1",
    )
    .bind(draw_date)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|err| {
        error!("Failed to check existing draw: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if existing_draw.is_some() {
        tx.rollback().await.ok();
        info!("Draw already exists for date {}, skipping import", draw_date);
        return Ok(Json(serde_json::json!({
            "message": "Draw already exists, skipping import",
            "draw_date": draw_date.to_string(),
            "status": "skipped",
            "prize_types_imported": 0,
            "prize_numbers_imported": 0
        })));
    }

    // // Insert lottery_draw
    sqlx::query(
        "INSERT INTO lottery_draw (id, draw_date) VALUES ($1, $2)",
    )
    .bind(draw_id)
    .bind(draw_date)
    .execute(&mut *tx)
    .await
    .map_err(|err| {
        error!("Failed to insert lottery_draw: {}", err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut prize_types_imported = 0;
    let mut prize_numbers_imported = 0;

    // // Process prize data
    if let Some(data) = response.get("data").and_then(|d| d.as_object()) {
        if data.is_empty() {
            info!("No prize data to import for draw {}", draw_date);
        }
        for (prize_code, prize_value) in data {
            // Validate prize structure
            let price_str = prize_value
                .get("price")
                .and_then(|p| p.as_str())
                .unwrap_or("0");

            // Try to parse price as i64 for validation
            if price_str.parse::<i64>().is_err() {
                error!("Invalid price '{}' for prize_code {}, using '0'", price_str, prize_code);
            }

            // Upsert prize_type
            let prize_type_id: i32 = sqlx::query_scalar(
                "INSERT INTO prize_type (code, prize_amount)
                 VALUES ($1, $2)
                 ON CONFLICT (code) DO UPDATE SET prize_amount = EXCLUDED.prize_amount
                 RETURNING id",
            )
            .bind(prize_code)
            .bind(price_str)
            .fetch_one(&mut *tx)
            .await
            .map_err(|err| {
                error!("Failed to upsert prize_type '{}': {}", prize_code, err);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            prize_types_imported += 1;

            // Insert prize numbers
            if let Some(numbers) = prize_value.get("number").and_then(|n| n.as_array()) {
                for number_obj in numbers {
                    let round = number_obj
                        .get("round")
                        .and_then(|r| r.as_i64())
                        .unwrap_or(0) as i32; // Cast to i32 for SQL
                    let value = number_obj
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    sqlx::query(
                        "INSERT INTO prize_number (draw_id, prize_type_id, round, number)
                         VALUES ($1, $2, $3, $4)
                         ON CONFLICT (draw_id, prize_type_id, round, number) DO NOTHING",
                    )
                    .bind(draw_id)
                    .bind(prize_type_id)
                    .bind(round)
                    .bind(value)
                    .execute(&mut *tx)
                    .await
                    .map_err(|err| {
                        error!("Failed to insert prize_number: {}", err);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;
                }
                prize_numbers_imported += numbers.len() as i32;
            }
        }
    } else {
        info!("No 'data' field in response for draw {}", draw_date);
    }

    // // Commit transaction
    tx.commit()
        .await
        .map_err(|err| {
            error!("Failed to commit transaction: {}", err);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Import successful for draw {}, prize_types: {}, prize_numbers: {}", draw_date, prize_types_imported, prize_numbers_imported);

    Ok(Json(serde_json::json!({
        "message": "Import successful",
        "draw_id": draw_id,
        "draw_date": draw_date,
        "status": "success",
        "prize_types_imported": prize_types_imported,
        "prize_numbers_imported": prize_numbers_imported
    })))
}

async fn check_postgres_handler() -> Result<Json<ApiResponse>, StatusCode> {
    dotenvy::dotenv().ok();

    let host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let user = env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
    let password = env::var("DB_PASS").unwrap_or_else(|_| "123456".to_string());
    let dbname = env::var("DB_NAME").unwrap_or_else(|_| "postgres".to_string());

    let conn_str = format!(
        "host={} port={} user={} password={} dbname={}",
        host, port, user, password, dbname
    );

    let (client, connection) = tokio_postgres::connect(&conn_str, NoTls)
        .await
        .map_err(|err| {
            eprintln!("Postgres connect failed: {}", err);
            StatusCode::BAD_GATEWAY
        })?;

    tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("Postgres connection error: {}", err);
        }
    });

    client
        .query_one("SELECT 1", &[])
        .await
        .map_err(|err| {
            eprintln!("Postgres query failed: {}", err);
            StatusCode::BAD_GATEWAY
        })?;

    Ok(Json(ApiResponse {
        message: "Postgres connected successfully".to_string(),
        source: serde_json::json!({
            "db_host": host,
            "db_port": port,
            "db_name": dbname,
        }),
    }))
}



async fn httpbin_handler(Query(params): Query<HttpBinQuery>) -> Result<Json<ApiResponse>, StatusCode> {
    let name = params.name.unwrap_or_else(|| "guest".to_string());
    let client = reqwest::Client::new();
    
    let resp = client
        .get("https://httpbin.org/get")
        .query(&[("name", &name)])
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let body: serde_json::Value = resp.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(ApiResponse {
        message: format!("Hello from Rust, {}", name),
        source: body,
    }))
}