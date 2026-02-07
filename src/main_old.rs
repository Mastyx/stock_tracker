// main.rs
use chrono::{DateTime, Utc};
use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
struct StockData {
    symbol: String,
    prices: Vec<f32>,
    timestamps: Vec<DateTime<Utc>>,
    current_price: f32,
    change_percent: f32,
}

#[derive(Debug, Clone)]
struct App {
    stocks: Arc<Mutex<HashMap<String, StockData>>>,
    last_update: Arc<Mutex<DateTime<Utc>>>,
    selected_symbols: HashSet<String>,
}

impl App {
    fn new() -> Self {
        Self {
            stocks: Arc::new(Mutex::new(HashMap::new())),
            last_update: Arc::new(Mutex::new(Utc::now())),
            selected_symbols: HashSet::new(),
        }
    }
}

// Funzione per ottenere dati da Yahoo Finance
fn fetch_stock_data(symbol: &str) -> Result<(f32, f32), Box<dyn std::error::Error>> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1m&range=1d",
        symbol
    );

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .timeout(Duration::from_secs(10))
        .send()?;

    let json: serde_json::Value = response.json()?;

    if let Some(result) = json["chart"]["result"][0].as_object() {
        if let Some(meta) = result["meta"].as_object() {
            let current_price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0) as f32;
            let prev_close = meta["chartPreviousClose"]
                .as_f64()
                .unwrap_or(current_price as f64) as f32;

            let change_percent = ((current_price - prev_close) / prev_close) * 100.0;

            return Ok((current_price, change_percent));
        }
    }

    Err("Failed to parse stock data".into())
}

// Disegna un singolo elemento della lista con checkbox
fn draw_list_item(
    stock: &StockData,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    is_selected: bool,
    mouse_pos: (f32, f32),
) -> bool {
    let (mx, my) = mouse_pos;
    let is_hovered = mx >= x && mx <= x + width && my >= y && my <= y + height;
    let is_clicked = is_hovered && is_mouse_button_pressed(MouseButton::Left);

    // Colore di sfondo
    let bg_color = if is_hovered {
        Color::from_rgba(40, 40, 50, 255)
    } else {
        Color::from_rgba(30, 30, 40, 255)
    };

    draw_rectangle(x, y, width, height, bg_color);

    // Bordo
    draw_rectangle_lines(x, y, width, height, 1.0, Color::from_rgba(50, 50, 60, 255));

    // Checkbox
    let checkbox_size = 20.0;
    let checkbox_x = x + 10.0;
    let checkbox_y = y + (height - checkbox_size) / 2.0;

    // Sfondo checkbox
    let checkbox_color = if is_selected {
        Color::from_rgba(50, 100, 200, 255)
    } else {
        Color::from_rgba(40, 40, 50, 255)
    };
    draw_rectangle(
        checkbox_x,
        checkbox_y,
        checkbox_size,
        checkbox_size,
        checkbox_color,
    );
    draw_rectangle_lines(
        checkbox_x,
        checkbox_y,
        checkbox_size,
        checkbox_size,
        2.0,
        Color::from_rgba(100, 100, 120, 255),
    );

    // Checkmark
    if is_selected {
        draw_line(
            checkbox_x + 4.0,
            checkbox_y + 10.0,
            checkbox_x + 8.0,
            checkbox_y + 16.0,
            2.0,
            WHITE,
        );
        draw_line(
            checkbox_x + 8.0,
            checkbox_y + 16.0,
            checkbox_x + 16.0,
            checkbox_y + 4.0,
            2.0,
            WHITE,
        );
    }

    // Simbolo
    draw_text(&stock.symbol, x + 45.0, y + 25.0, 20.0, WHITE);

    // Prezzo
    if stock.current_price > 0.0 {
        let price_text = format!("${:.2}", stock.current_price);
        draw_text(&price_text, x + 45.0, y + 48.0, 16.0, LIGHTGRAY);

        // Variazione percentuale
        let change_color = if stock.change_percent >= 0.0 {
            Color::from_rgba(0, 200, 100, 255)
        } else {
            Color::from_rgba(220, 50, 50, 255)
        };
        let change_text = format!("{:+.2}%", stock.change_percent);
        draw_text(&change_text, x + width - 80.0, y + 35.0, 18.0, change_color);
    } else {
        draw_text("Caricamento...", x + 45.0, y + 48.0, 14.0, GRAY);
    }

    is_clicked
}

// Disegna il pannello lista a sinistra
fn draw_list_panel(
    stocks_snapshot: &HashMap<String, StockData>,
    symbols: &[String],
    selected_symbols: &HashSet<String>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scroll_offset: f32,
) -> (Option<String>, f32) {
    // Sfondo del pannello
    draw_rectangle(x, y, width, height, Color::from_rgba(25, 25, 35, 255));

    // Titolo
    draw_text("TITOLI", x + 10.0, y + 30.0, 24.0, WHITE);

    // Contatore selezioni
    let count_text = format!("Selezionati: {}", selected_symbols.len());
    draw_text(&count_text, x + width - 120.0, y + 30.0, 18.0, LIGHTGRAY);

    draw_line(
        x,
        y + 40.0,
        x + width,
        y + 40.0,
        2.0,
        Color::from_rgba(50, 50, 60, 255),
    );

    let item_height = 70.0;
    let item_padding = 5.0;
    let start_y = y + 50.0;
    let mouse_pos = mouse_position();

    let mut clicked_symbol = None;
    let mut new_scroll = scroll_offset;

    // Gestione scroll con mouse wheel
    let (_wheel_x, wheel_y) = mouse_wheel();
    if mouse_pos.0 >= x && mouse_pos.0 <= x + width && mouse_pos.1 >= y && mouse_pos.1 <= y + height
    {
        new_scroll += wheel_y * 20.0;
        new_scroll = new_scroll.max(0.0);
    }

    // Calcola area visibile
    let visible_area_y = start_y;
    let visible_area_height = height - 50.0;

    for (i, symbol) in symbols.iter().enumerate() {
        if let Some(stock) = stocks_snapshot.get(symbol) {
            let item_y = visible_area_y + (i as f32) * (item_height + item_padding) - new_scroll;

            // Disegna solo se visibile
            if item_y + item_height >= visible_area_y
                && item_y <= visible_area_y + visible_area_height
            {
                let is_selected = selected_symbols.contains(symbol);

                if draw_list_item(
                    stock,
                    x + 10.0,
                    item_y,
                    width - 20.0,
                    item_height,
                    is_selected,
                    mouse_pos,
                ) {
                    clicked_symbol = Some(symbol.clone());
                }
            }
        }
    }

    (clicked_symbol, new_scroll)
}

// Disegna un mini-grafico per la vista multipla
fn draw_mini_chart(stock: &StockData, x: f32, y: f32, width: f32, height: f32) {
    // Sfondo
    draw_rectangle(x, y, width, height, Color::from_rgba(30, 30, 40, 255));
    draw_rectangle_lines(x, y, width, height, 2.0, Color::from_rgba(50, 50, 60, 255));

    // Header
    draw_text(&stock.symbol, x + 15.0, y + 28.0, 24.0, WHITE);

    let price_text = format!("${:.2}", stock.current_price);
    draw_text(&price_text, x + 15.0, y + 52.0, 20.0, LIGHTGRAY);

    let change_color = if stock.change_percent >= 0.0 {
        Color::from_rgba(0, 200, 100, 255)
    } else {
        Color::from_rgba(220, 50, 50, 255)
    };
    let change_text = format!("{:+.2}%", stock.change_percent);
    draw_text(&change_text, x + width - 80.0, y + 40.0, 20.0, change_color);

    if stock.prices.len() < 2 {
        draw_text(
            "Caricamento...",
            x + width / 2.0 - 50.0,
            y + height / 2.0 + 20.0,
            16.0,
            GRAY,
        );
        return;
    }

    // Area del grafico
    let chart_x = x + 20.0;
    let chart_y = y + 70.0;
    let chart_width = width - 40.0;
    let chart_height = height - 100.0;

    // Trova min e max
    let min_price = stock.prices.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_price = stock
        .prices
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let price_range = max_price - min_price;

    if price_range == 0.0 {
        return;
    }

    // Colore della linea
    let line_color = if stock.change_percent >= 0.0 {
        Color::from_rgba(0, 220, 120, 255)
    } else {
        Color::from_rgba(255, 80, 80, 255)
    };

    // Disegna area sotto la linea
    for i in 0..stock.prices.len() - 1 {
        let x1 = chart_x + (i as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y1 =
            chart_y + chart_height - ((stock.prices[i] - min_price) / price_range) * chart_height;

        let x2 = chart_x + ((i + 1) as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y2 = chart_y + chart_height
            - ((stock.prices[i + 1] - min_price) / price_range) * chart_height;

        let mut fill_color = line_color;
        fill_color.a = 0.15;
        draw_triangle(
            vec2(x1, y1),
            vec2(x2, y2),
            vec2(x2, chart_y + chart_height),
            fill_color,
        );
        draw_triangle(
            vec2(x1, y1),
            vec2(x2, chart_y + chart_height),
            vec2(x1, chart_y + chart_height),
            fill_color,
        );
    }

    // Disegna la linea
    for i in 0..stock.prices.len() - 1 {
        let x1 = chart_x + (i as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y1 =
            chart_y + chart_height - ((stock.prices[i] - min_price) / price_range) * chart_height;

        let x2 = chart_x + ((i + 1) as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y2 = chart_y + chart_height
            - ((stock.prices[i + 1] - min_price) / price_range) * chart_height;

        draw_line(x1, y1, x2, y2, 2.0, line_color);
    }

    // Mini stats
    draw_text(
        &format!("H: ${:.2}", max_price),
        x + 15.0,
        y + height - 10.0,
        14.0,
        GRAY,
    );
    draw_text(
        &format!("L: ${:.2}", min_price),
        x + width - 80.0,
        y + height - 10.0,
        14.0,
        GRAY,
    );
}

// Disegna il pannello con i grafici selezionati a destra
fn draw_charts_panel(
    stocks_snapshot: &HashMap<String, StockData>,
    selected_symbols: &HashSet<String>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Sfondo
    draw_rectangle(x, y, width, height, Color::from_rgba(20, 20, 30, 255));

    if selected_symbols.is_empty() {
        draw_text(
            "Seleziona uno o più titoli dalla lista a sinistra",
            x + width / 2.0 - 200.0,
            y + height / 2.0,
            20.0,
            GRAY,
        );
        return;
    }

    // Calcola layout in griglia
    let selected_vec: Vec<&String> = selected_symbols.iter().collect();
    let count = selected_vec.len();

    let cols = if count == 1 {
        1
    } else if count <= 4 {
        2
    } else {
        3
    };
    let rows = (count + cols - 1) / cols;

    let padding = 15.0;
    let chart_width = (width - padding * (cols as f32 + 1.0)) / cols as f32;
    let chart_height = (height - padding * (rows as f32 + 1.0)) / rows as f32;

    for (i, symbol) in selected_vec.iter().enumerate() {
        if let Some(stock) = stocks_snapshot.get(*symbol) {
            let col = i % cols;
            let row = i / cols;

            let chart_x = x + padding + col as f32 * (chart_width + padding);
            let chart_y = y + padding + row as f32 * (chart_height + padding);

            draw_mini_chart(stock, chart_x, chart_y, chart_width, chart_height);
        }
    }
}

// Thread worker per aggiornare i dati
fn start_update_worker(
    stocks: Arc<Mutex<HashMap<String, StockData>>>,
    last_update: Arc<Mutex<DateTime<Utc>>>,
    symbols: Vec<String>,
) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(60));

            let now = Utc::now();

            for symbol in &symbols {
                match fetch_stock_data(symbol) {
                    Ok((price, change)) => {
                        if let Ok(mut stocks_lock) = stocks.lock() {
                            if let Some(stock) = stocks_lock.get_mut(symbol) {
                                stock.prices.push(price);
                                stock.timestamps.push(now);
                                stock.current_price = price;
                                stock.change_percent = change;

                                if stock.prices.len() > 60 {
                                    stock.prices.remove(0);
                                    stock.timestamps.remove(0);
                                }

                                println!("{}: ${:.2} ({:+.2}%)", symbol, price, change);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Errore nel recupero dati per {}: {}", symbol, e);
                    }
                }
            }

            if let Ok(mut last_update_lock) = last_update.lock() {
                *last_update_lock = now;
            }
        }
    });
}

// Thread per il fetch iniziale
fn initial_fetch(stocks: Arc<Mutex<HashMap<String, StockData>>>, symbols: Vec<String>) {
    thread::spawn(move || {
        for symbol in &symbols {
            match fetch_stock_data(symbol) {
                Ok((price, change)) => {
                    if let Ok(mut stocks_lock) = stocks.lock() {
                        if let Some(stock) = stocks_lock.get_mut(symbol) {
                            stock.current_price = price;
                            stock.change_percent = change;
                            stock.prices.push(price);
                            stock.timestamps.push(Utc::now());
                            println!("Caricato {}: ${:.2} ({:+.2}%)", symbol, price, change);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Errore nel caricamento iniziale per {}: {}", symbol, e);
                }
            }
        }
    });
}

#[macroquad::main("Stock Tracker - Multi Selection")]
async fn main() {
    let mut app = App::new();
    let mut scroll_offset = 0.0;

    // Lista estesa di titoli ed ETF
    let symbols = vec![
        "SPY".to_string(),
        "QQQ".to_string(),
        "DIA".to_string(),
        "IWM".to_string(),
        "AAPL".to_string(),
        "MSFT".to_string(),
        "GOOGL".to_string(),
        "AMZN".to_string(),
        "TSLA".to_string(),
        "NVDA".to_string(),
        "META".to_string(),
        "NFLX".to_string(),
        "AMD".to_string(),
        "INTC".to_string(),
        "JPM".to_string(),
        "BAC".to_string(),
        "WMT".to_string(),
        "V".to_string(),
        "MA".to_string(),
        "XOM".to_string(),
        "BTC-USD".to_string(),
        "ETH-USD".to_string(),   // Ethereum
        "BNB-USD".to_string(),   // Binance Coin
        "XRP-USD".to_string(),   // Ripple
        "ADA-USD".to_string(),   // Cardano
        "SOL-USD".to_string(),   // Solana
        "DOGE-USD".to_string(),  // Dogecoin
        "DOT-USD".to_string(),   // Polkadot
        "MATIC-USD".to_string(), // Polygon
        "AVAX-USD".to_string(),  // Avalanche
        "LINK-USD".to_string(),  // Chainlink
        "UNI-USD".to_string(),   // Uniswap
        "LTC-USD".to_string(),   // Litecoin
        "ATOM-USD".to_string(),  // Cosmos
        "XLM-USD".to_string(),   // Stellar
    ];

    // Inizializza le strutture dati
    {
        let mut stocks = app.stocks.lock().unwrap();
        for symbol in &symbols {
            stocks.insert(
                symbol.clone(),
                StockData {
                    symbol: symbol.clone(),
                    prices: Vec::new(),
                    timestamps: Vec::new(),
                    current_price: 0.0,
                    change_percent: 0.0,
                },
            );
        }
    }

    // Avvia fetch iniziale
    initial_fetch(app.stocks.clone(), symbols.clone());

    // Avvia worker per aggiornamenti periodici
    start_update_worker(app.stocks.clone(), app.last_update.clone(), symbols.clone());

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        clear_background(Color::from_rgba(20, 20, 30, 255));

        let screen_w = screen_width();
        let screen_h = screen_height();

        // Dimensioni pannelli
        let list_width = 320.0;
        let charts_width = screen_w - list_width;

        // Copia i dati per il rendering
        let stocks_snapshot = { app.stocks.lock().unwrap().clone() };

        // Disegna pannello lista a sinistra
        let (clicked, new_scroll) = draw_list_panel(
            &stocks_snapshot,
            &symbols,
            &app.selected_symbols,
            0.0,
            0.0,
            list_width,
            screen_h,
            scroll_offset,
        );

        scroll_offset = new_scroll;

        // Gestisci toggle selezione
        if let Some(clicked_symbol) = clicked {
            if app.selected_symbols.contains(&clicked_symbol) {
                app.selected_symbols.remove(&clicked_symbol);
            } else {
                app.selected_symbols.insert(clicked_symbol);
            }
        }

        // Linea separatrice verticale
        draw_line(
            list_width,
            0.0,
            list_width,
            screen_h,
            2.0,
            Color::from_rgba(50, 50, 60, 255),
        );

        // Disegna grafici selezionati a destra
        draw_charts_panel(
            &stocks_snapshot,
            &app.selected_symbols,
            list_width,
            0.0,
            charts_width,
            screen_h,
        );

        // Info ultimo aggiornamento
        let last_update = { *app.last_update.lock().unwrap() };
        let update_text = format!("Aggiornamento: {}", last_update.format("%H:%M:%S"));
        draw_text(&update_text, list_width + 15.0, screen_h - 10.0, 14.0, GRAY);

        next_frame().await
    }
}
