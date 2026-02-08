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
    prices_24h_ago: Vec<f32>,
    timestamps_24h_ago: Vec<DateTime<Utc>>,
    current_price: f32,
    change_percent: f32,
    change_24h_percent: f32,
}

#[derive(Debug, Clone)]
struct ScrollbarState {
    dragging: bool,
    drag_start_y: f32,
    drag_start_scroll: f32,
}

impl ScrollbarState {
    fn new() -> Self {
        Self {
            dragging: false,
            drag_start_y: 0.0,
            drag_start_scroll: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct App {
    stocks: Arc<Mutex<HashMap<String, StockData>>>,
    last_update: Arc<Mutex<DateTime<Utc>>>,
    selected_symbols: HashSet<String>,
    scrollbar_state: ScrollbarState,
}

impl App {
    fn new() -> Self {
        Self {
            stocks: Arc::new(Mutex::new(HashMap::new())),
            last_update: Arc::new(Mutex::new(Utc::now())),
            selected_symbols: HashSet::new(),
            scrollbar_state: ScrollbarState::new(),
        }
    }
}

// Funzione per ottenere dati attuali
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

// Recupera dati storici delle ultime 24 ore
fn fetch_24h_historical_data(
    symbol: &str,
) -> Result<(Vec<f32>, Vec<DateTime<Utc>>), Box<dyn std::error::Error>> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=5m&range=1d",
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
        let timestamps = result["timestamp"].as_array().ok_or("No timestamps")?;

        let quotes = result["indicators"]["quote"][0]
            .as_object()
            .ok_or("No quotes")?;

        let closes = quotes["close"].as_array().ok_or("No close prices")?;

        let mut prices = Vec::new();
        let mut times = Vec::new();

        for (i, ts) in timestamps.iter().enumerate() {
            if let Some(timestamp) = ts.as_i64() {
                if let Some(price_val) = closes.get(i) {
                    if let Some(price) = price_val.as_f64() {
                        let dt = DateTime::from_timestamp(timestamp, 0).unwrap_or(Utc::now());
                        times.push(dt);
                        prices.push(price as f32);
                    }
                }
            }
        }

        return Ok((prices, times));
    }

    Err("Failed to parse historical data".into())
}

// Disegna la scrollbar
fn draw_scrollbar(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scroll_offset: f32,
    total_content_height: f32,
    visible_height: f32,
    scrollbar_state: &mut ScrollbarState,
) -> f32 {
    let scrollbar_width = 12.0;
    let scrollbar_x = x + width - scrollbar_width - 5.0;
    let scrollbar_y = y;

    // Sfondo della scrollbar
    draw_rectangle(
        scrollbar_x,
        scrollbar_y,
        scrollbar_width,
        height,
        Color::from_rgba(20, 20, 25, 255),
    );

    // Bordo sottile
    draw_rectangle_lines(
        scrollbar_x,
        scrollbar_y,
        scrollbar_width,
        height,
        1.0,
        Color::from_rgba(40, 40, 50, 255),
    );

    let max_scroll = (total_content_height - visible_height).max(0.0);

    if max_scroll <= 0.0 {
        return scroll_offset;
    }

    // Calcola thumb
    let thumb_height = ((visible_height / total_content_height) * height)
        .max(30.0)
        .min(height - 10.0);
    let scroll_range = height - thumb_height;
    let thumb_y = scrollbar_y + (scroll_offset / max_scroll) * scroll_range;

    let mouse_pos = mouse_position();
    let mx = mouse_pos.0;
    let my = mouse_pos.1;

    let is_on_scrollbar = mx >= scrollbar_x
        && mx <= scrollbar_x + scrollbar_width
        && my >= scrollbar_y
        && my <= scrollbar_y + height;

    let is_on_thumb = mx >= scrollbar_x
        && mx <= scrollbar_x + scrollbar_width
        && my >= thumb_y
        && my <= thumb_y + thumb_height;

    // Colore dinamico
    let thumb_color = if scrollbar_state.dragging {
        Color::from_rgba(120, 120, 140, 255)
    } else if is_on_thumb {
        Color::from_rgba(100, 100, 120, 255)
    } else if is_on_scrollbar {
        Color::from_rgba(80, 80, 100, 255)
    } else {
        Color::from_rgba(60, 60, 80, 255)
    };

    // Disegna thumb
    draw_rectangle(
        scrollbar_x + 2.0,
        thumb_y + 2.0,
        scrollbar_width - 4.0,
        thumb_height - 4.0,
        thumb_color,
    );

    // Indicatori sul thumb (tre linee)
    let indicator_spacing = 2.0;
    let indicator_y = thumb_y + thumb_height / 2.0;
    for i in 0..3 {
        let line_y = indicator_y - indicator_spacing + (i as f32 * indicator_spacing);
        draw_line(
            scrollbar_x + 4.0,
            line_y,
            scrollbar_x + scrollbar_width - 4.0,
            line_y,
            1.0,
            Color::from_rgba(40, 40, 50, 200),
        );
    }

    // Gestione interazione
    if is_mouse_button_pressed(MouseButton::Left) && is_on_thumb {
        scrollbar_state.dragging = true;
        scrollbar_state.drag_start_y = my;
        scrollbar_state.drag_start_scroll = scroll_offset;
    }

    if is_mouse_button_released(MouseButton::Left) {
        scrollbar_state.dragging = false;
    }

    if scrollbar_state.dragging {
        let delta_y = my - scrollbar_state.drag_start_y;
        let scroll_delta = (delta_y / scroll_range) * max_scroll;
        let new_scroll = (scrollbar_state.drag_start_scroll + scroll_delta)
            .max(0.0)
            .min(max_scroll);
        return new_scroll;
    }

    // Click sulla track
    if is_mouse_button_pressed(MouseButton::Left) && is_on_scrollbar && !is_on_thumb {
        let click_ratio = (my - scrollbar_y) / height;
        let new_scroll = (click_ratio * max_scroll).max(0.0).min(max_scroll);
        return new_scroll;
    }

    scroll_offset
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

    let bg_color = if is_hovered {
        Color::from_rgba(40, 40, 50, 255)
    } else {
        Color::from_rgba(30, 30, 40, 255)
    };

    draw_rectangle(x, y, width, height, bg_color);
    draw_rectangle_lines(x, y, width, height, 1.0, Color::from_rgba(50, 50, 60, 255));

    // Checkbox
    let checkbox_size = 20.0;
    let checkbox_x = x + 10.0;
    let checkbox_y = y + (height - checkbox_size) / 2.0;

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
    draw_text(&stock.symbol, x + 45.0, y + 22.0, 20.0, WHITE);

    // Prezzo
    if stock.current_price > 0.0 {
        let price_text = format!("${:.2}", stock.current_price);
        draw_text(&price_text, x + 45.0, y + 42.0, 20.0, LIGHTGRAY);

        // Variazione giornaliera
        let change_color = if stock.change_percent >= 0.0 {
            Color::from_rgba(0, 200, 100, 255)
        } else {
            Color::from_rgba(220, 50, 50, 255)
        };
        let change_text = format!("{:+.2}%", stock.change_percent);
        draw_text(&change_text, x + 45.0, y + 60.0, 16.0, change_color);

        // Variazione 24h
        if stock.change_24h_percent != 0.0 {
            let change_24h_color = Color::from_rgba(100, 150, 255, 255);
            let change_24h_text = format!("24h: {:+.2}%", stock.change_24h_percent);
            draw_text(
                &change_24h_text,
                x + width - 90.0,
                y + 60.0,
                16.0,
                change_24h_color,
            );
        }
    } else {
        draw_text("Caricamento...", x + 45.0, y + 48.0, 16.0, GRAY);
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
    scrollbar_state: &mut ScrollbarState,
) -> (Option<String>, f32) {
    draw_rectangle(x, y, width, height, Color::from_rgba(25, 25, 35, 255));

    draw_text("TITOLI", x + 10.0, y + 30.0, 24.0, WHITE);

    let count_text = format!("Selezionati: {}", selected_symbols.len());
    draw_text(&count_text, x + width - 130.0, y + 30.0, 18.0, LIGHTGRAY);

    draw_line(
        x,
        y + 40.0,
        x + width,
        y + 40.0,
        2.0,
        Color::from_rgba(50, 50, 60, 255),
    );

    let item_height = 75.0;
    let item_padding = 5.0;
    let start_y = y + 50.0;
    let mouse_pos = mouse_position();

    let mut clicked_symbol = None;
    let mut new_scroll = scroll_offset;

    let (_wheel_x, wheel_y) = mouse_wheel();
    let mouse_in_list = mouse_pos.0 >= x
        && mouse_pos.0 <= x + width
        && mouse_pos.1 >= y
        && mouse_pos.1 <= y + height;

    if mouse_in_list && !scrollbar_state.dragging {
        new_scroll += wheel_y * 20.0;

        let total_items_height = symbols.len() as f32 * (item_height + item_padding);
        let visible_height = height - 50.0;
        let max_scroll = (total_items_height - visible_height).max(0.0);

        new_scroll = new_scroll.max(0.0).min(max_scroll);
    }

    let visible_area_y = start_y;
    let visible_area_height = height - 50.0;
    let items_width = width - 30.0;

    for (i, symbol) in symbols.iter().enumerate() {
        if let Some(stock) = stocks_snapshot.get(symbol) {
            let item_y = visible_area_y + (i as f32) * (item_height + item_padding) - new_scroll;

            if item_y + item_height >= visible_area_y
                && item_y <= visible_area_y + visible_area_height
            {
                let is_selected = selected_symbols.contains(symbol);

                if draw_list_item(
                    stock,
                    x + 10.0,
                    item_y,
                    items_width - 20.0,
                    item_height,
                    is_selected,
                    mouse_pos,
                ) {
                    clicked_symbol = Some(symbol.clone());
                }
            }
        }
    }

    // Disegna scrollbar
    let total_content_height = symbols.len() as f32 * (item_height + item_padding);

    new_scroll = draw_scrollbar(
        x,
        start_y,
        width,
        visible_area_height,
        new_scroll,
        total_content_height,
        visible_area_height,
        scrollbar_state,
    );

    (clicked_symbol, new_scroll)
}

// Disegna mini-grafico con confronto 24h
// Sostituisci la funzione draw_mini_chart con questa nuova versione a doppio layer
fn draw_mini_chart(stock: &StockData, x: f32, y: f32, width: f32, height: f32) {
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
    draw_text(&change_text, x + width - 90.0, y + 32.0, 20.0, change_color);

    // Mostra anche variazione 24h
    if stock.change_24h_percent != 0.0 {
        let change_24h_text = format!("24h: {:+.2}%", stock.change_24h_percent);
        draw_text(
            &change_24h_text,
            x + width - 90.0,
            y + 52.0,
            16.0,
            Color::from_rgba(100, 150, 255, 255),
        );
    }

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
    let chart_x = x + 70.0; // Più spazio a sinistra per le etichette
    let chart_y = y + 75.0;
    let chart_width = width - 90.0;
    let chart_height = height - 105.0;

    // LAYER 1: GRAFICO 24H FA (BLU) - Con la sua scala indipendente
    if stock.prices_24h_ago.len() >= 2 {
        let h24_min = stock
            .prices_24h_ago
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let h24_max = stock
            .prices_24h_ago
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let h24_range = h24_max - h24_min;

        if h24_range > 0.0 {
            // Padding 8%
            let h24_padding = h24_range * 0.08;
            let h24_chart_min = h24_min - h24_padding;
            let h24_chart_max = h24_max + h24_padding;
            let h24_chart_range = h24_chart_max - h24_chart_min;

            let blue_color = Color::from_rgba(100, 150, 255, 150);

            // Area riempita sotto la linea blu
            for i in 0..stock.prices_24h_ago.len() - 1 {
                let x1 =
                    chart_x + (i as f32 / (stock.prices_24h_ago.len() - 1) as f32) * chart_width;
                let y1 = chart_y + chart_height
                    - ((stock.prices_24h_ago[i] - h24_chart_min) / h24_chart_range) * chart_height;

                let x2 = chart_x
                    + ((i + 1) as f32 / (stock.prices_24h_ago.len() - 1) as f32) * chart_width;
                let y2 = chart_y + chart_height
                    - ((stock.prices_24h_ago[i + 1] - h24_chart_min) / h24_chart_range)
                        * chart_height;

                let mut fill_blue = blue_color;
                fill_blue.a = 0.1;
                draw_triangle(
                    vec2(x1, y1),
                    vec2(x2, y2),
                    vec2(x2, chart_y + chart_height),
                    fill_blue,
                );
                draw_triangle(
                    vec2(x1, y1),
                    vec2(x2, chart_y + chart_height),
                    vec2(x1, chart_y + chart_height),
                    fill_blue,
                );
            }

            // Linea blu
            for i in 0..stock.prices_24h_ago.len() - 1 {
                let x1 =
                    chart_x + (i as f32 / (stock.prices_24h_ago.len() - 1) as f32) * chart_width;
                let y1 = chart_y + chart_height
                    - ((stock.prices_24h_ago[i] - h24_chart_min) / h24_chart_range) * chart_height;

                let x2 = chart_x
                    + ((i + 1) as f32 / (stock.prices_24h_ago.len() - 1) as f32) * chart_width;
                let y2 = chart_y + chart_height
                    - ((stock.prices_24h_ago[i + 1] - h24_chart_min) / h24_chart_range)
                        * chart_height;

                draw_line(x1, y1, x2, y2, 2.2, blue_color);
            }

            // Etichette scala BLU (sul lato sinistro)
            for i in 0..3 {
                let grid_y = chart_y + (i as f32 / 2.0) * chart_height;
                let price_at_line = h24_chart_max - (i as f32 / 2.0) * h24_chart_range;

                draw_text(
                    &format!("${:.2}", price_at_line),
                    x + 5.0,
                    grid_y + 5.0,
                    13.0,
                    Color::from_rgba(100, 150, 255, 180),
                );
            }
        }
    }

    // LAYER 2: GRAFICO CORRENTE - Con la sua scala indipendente
    let current_min = stock.prices.iter().cloned().fold(f32::INFINITY, f32::min);
    let current_max = stock
        .prices
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let current_range = current_max - current_min;

    if current_range == 0.0 {
        return;
    }

    // Padding 8%
    let current_padding = current_range * 0.08;
    let current_chart_min = current_min - current_padding;
    let current_chart_max = current_max + current_padding;
    let current_chart_range = current_chart_max - current_chart_min;

    // Griglia orizzontale leggera
    for i in 0..5 {
        let grid_y = chart_y + (i as f32 / 4.0) * chart_height;
        draw_line(
            chart_x,
            grid_y,
            chart_x + chart_width,
            grid_y,
            0.5,
            Color::from_rgba(40, 40, 50, 100),
        );
    }

    // Colore della linea corrente
    let line_color = if stock.change_percent >= 0.0 {
        Color::from_rgba(0, 220, 120, 255)
    } else {
        Color::from_rgba(255, 80, 80, 255)
    };

    // Area sotto la linea corrente
    for i in 0..stock.prices.len() - 1 {
        let x1 = chart_x + (i as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y1 = chart_y + chart_height
            - ((stock.prices[i] - current_chart_min) / current_chart_range) * chart_height;

        let x2 = chart_x + ((i + 1) as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y2 = chart_y + chart_height
            - ((stock.prices[i + 1] - current_chart_min) / current_chart_range) * chart_height;

        let mut fill_color = line_color;
        fill_color.a = 0.25;
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

    // Linea corrente (la più spessa e visibile)
    for i in 0..stock.prices.len() - 1 {
        let x1 = chart_x + (i as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y1 = chart_y + chart_height
            - ((stock.prices[i] - current_chart_min) / current_chart_range) * chart_height;

        let x2 = chart_x + ((i + 1) as f32 / (stock.prices.len() - 1) as f32) * chart_width;
        let y2 = chart_y + chart_height
            - ((stock.prices[i + 1] - current_chart_min) / current_chart_range) * chart_height;

        draw_line(x1, y1, x2, y2, 3.0, line_color);
    }

    // Etichette scala CORRENTE (sul lato destro)
    for i in 0..5 {
        let grid_y = chart_y + (i as f32 / 4.0) * chart_height;
        let price_at_line = current_chart_max - (i as f32 / 4.0) * current_chart_range;

        // Sfondo semi-trasparente per leggibilità
        let label_text = format!("${:.2}", price_at_line);
        let text_width = 45.0;
        draw_rectangle(
            x + width - text_width - 8.0,
            grid_y - 8.0,
            text_width,
            14.0,
            Color::from_rgba(30, 30, 40, 200),
        );

        draw_text(
            &label_text,
            x + width - text_width - 5.0,
            grid_y + 4.0,
            13.0,
            line_color,
        );
    }

    // Legenda dettagliata
    let legend_y = y + height - 10.0;

    // Box sfondo legenda
    draw_rectangle(
        x + 8.0,
        legend_y - 22.0,
        width - 16.0,
        24.0,
        Color::from_rgba(20, 20, 25, 220),
    );
    draw_rectangle_lines(
        x + 8.0,
        legend_y - 22.0,
        width - 16.0,
        24.0,
        1.0,
        Color::from_rgba(50, 50, 60, 255),
    );

    // Linea corrente con info
    draw_line(
        x + 12.0,
        legend_y - 10.0,
        x + 30.0,
        legend_y - 10.0,
        3.0,
        line_color,
    );
    let current_info = format!("Ora (${:.2}-${:.2})", current_min, current_max);
    draw_text(&current_info, x + 35.0, legend_y - 5.0, 15.0, WHITE);

    // Linea 24h con info
    let mid_x = x + width / 2.0 + 10.0;
    draw_line(
        mid_x,
        legend_y - 10.0,
        mid_x + 18.0,
        legend_y - 10.0,
        2.2,
        Color::from_rgba(100, 150, 255, 255),
    );

    if stock.prices_24h_ago.len() >= 2 {
        let h24_min = stock
            .prices_24h_ago
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let h24_max = stock
            .prices_24h_ago
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let h24_info = format!("24h fa (${:.2}-${:.2})", h24_min, h24_max);
        draw_text(
            &h24_info,
            mid_x + 23.0,
            legend_y - 5.0,
            15.0,
            Color::from_rgba(150, 170, 200, 255),
        );
    } else {
        draw_text(
            "24h fa",
            mid_x + 23.0,
            legend_y - 5.0,
            11.0,
            Color::from_rgba(150, 170, 200, 255),
        );
    }
}

// Disegna il pannello con i grafici selezionati
// versione con altezza massima
fn draw_charts_panel(
    stocks_snapshot: &HashMap<String, StockData>,
    selected_symbols: &HashSet<String>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
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

    // NUOVA LOGICA: Calcola altezza con limite massimo di 500px per card
    let max_chart_height = 500.0;
    let available_chart_width = (width - padding * (cols as f32 + 1.0)) / cols as f32;
    let available_chart_height = (height - padding * (rows as f32 + 1.0)) / rows as f32;

    // Applica il limite massimo
    let chart_height = available_chart_height.min(max_chart_height);
    let chart_width = available_chart_width;

    // Calcola l'offset verticale per centrare se le card sono più piccole dell'area disponibile
    let total_charts_height = rows as f32 * chart_height + (rows as f32 + 1.0) * padding;
    let vertical_offset = if total_charts_height < height {
        (height - total_charts_height) / 2.0
    } else {
        0.0
    };

    for (i, symbol) in selected_vec.iter().enumerate() {
        if let Some(stock) = stocks_snapshot.get(*symbol) {
            let col = i % cols;
            let row = i / cols;

            let chart_x = x + padding + col as f32 * (chart_width + padding);
            let chart_y = y + padding + vertical_offset + row as f32 * (chart_height + padding);

            draw_mini_chart(stock, chart_x, chart_y, chart_width, chart_height);
        }
    }
}
// Thread worker per aggiornamenti ogni minuto
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

// Thread per aggiornare dati 24h ogni 5 minuti
fn start_24h_update_worker(stocks: Arc<Mutex<HashMap<String, StockData>>>, symbols: Vec<String>) {
    thread::spawn(move || {
        loop {
            for symbol in &symbols {
                match fetch_24h_historical_data(symbol) {
                    Ok((prices, timestamps)) => {
                        if let Ok(mut stocks_lock) = stocks.lock() {
                            if let Some(stock) = stocks_lock.get_mut(symbol) {
                                stock.prices_24h_ago = prices.clone();
                                stock.timestamps_24h_ago = timestamps;

                                if !prices.is_empty() && stock.current_price > 0.0 {
                                    let price_24h_ago = prices[0];
                                    stock.change_24h_percent =
                                        ((stock.current_price - price_24h_ago) / price_24h_ago)
                                            * 100.0;
                                }

                                println!(
                                    "{}: Aggiornati dati 24h ({} punti)",
                                    symbol,
                                    prices.len()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Errore nel recupero dati 24h per {}: {}", symbol, e);
                    }
                }
            }

            thread::sleep(Duration::from_secs(300));
        }
    });
}

// Thread per fetch iniziale
fn initial_fetch(stocks: Arc<Mutex<HashMap<String, StockData>>>, symbols: Vec<String>) {
    let stocks_clone = stocks.clone();
    let symbols_clone = symbols.clone();

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

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(2));

        for symbol in &symbols_clone {
            match fetch_24h_historical_data(symbol) {
                Ok((prices, timestamps)) => {
                    if let Ok(mut stocks_lock) = stocks_clone.lock() {
                        if let Some(stock) = stocks_lock.get_mut(symbol) {
                            stock.prices_24h_ago = prices.clone();
                            stock.timestamps_24h_ago = timestamps;

                            if !prices.is_empty() && stock.current_price > 0.0 {
                                let price_24h_ago = prices[0];
                                stock.change_24h_percent =
                                    ((stock.current_price - price_24h_ago) / price_24h_ago) * 100.0;
                            }

                            println!("{}: Caricati dati 24h ({} punti)", symbol, prices.len());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Errore caricamento 24h per {}: {}", symbol, e);
                }
            }
        }
    });
}

#[macroquad::main("Stock Tracker - Complete")]
async fn main() {
    let mut app = App::new();
    let mut scroll_offset = 0.0;

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
        "ETH-USD".to_string(),
        "BNB-USD".to_string(),
        "XRP-USD".to_string(),
        "ADA-USD".to_string(),
        "SOL-USD".to_string(),
        "DOGE-USD".to_string(),
        "DOT-USD".to_string(),
        "MATIC-USD".to_string(),
        "AVAX-USD".to_string(),
    ];

    {
        let mut stocks = app.stocks.lock().unwrap();
        for symbol in &symbols {
            stocks.insert(
                symbol.clone(),
                StockData {
                    symbol: symbol.clone(),
                    prices: Vec::new(),
                    timestamps: Vec::new(),
                    prices_24h_ago: Vec::new(),
                    timestamps_24h_ago: Vec::new(),
                    current_price: 0.0,
                    change_percent: 0.0,
                    change_24h_percent: 0.0,
                },
            );
        }
    }

    initial_fetch(app.stocks.clone(), symbols.clone());
    start_update_worker(app.stocks.clone(), app.last_update.clone(), symbols.clone());
    start_24h_update_worker(app.stocks.clone(), symbols.clone());

    loop {
        clear_background(Color::from_rgba(20, 20, 30, 255));
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        let screen_w = screen_width();
        let screen_h = screen_height();

        let list_width = 320.0;
        let charts_width = screen_w - list_width;

        let stocks_snapshot = { app.stocks.lock().unwrap().clone() };

        let (clicked, new_scroll) = draw_list_panel(
            &stocks_snapshot,
            &symbols,
            &app.selected_symbols,
            0.0,
            0.0,
            list_width,
            screen_h,
            scroll_offset,
            &mut app.scrollbar_state,
        );

        scroll_offset = new_scroll;

        if let Some(clicked_symbol) = clicked {
            if app.selected_symbols.contains(&clicked_symbol) {
                app.selected_symbols.remove(&clicked_symbol);
            } else {
                app.selected_symbols.insert(clicked_symbol);
            }
        }

        draw_line(
            list_width,
            0.0,
            list_width,
            screen_h,
            2.0,
            Color::from_rgba(50, 50, 60, 255),
        );

        draw_charts_panel(
            &stocks_snapshot,
            &app.selected_symbols,
            list_width,
            0.0,
            charts_width,
            screen_h,
        );

        let last_update = { *app.last_update.lock().unwrap() };
        let update_text = format!("Aggiornamento: {}", last_update.format("%H:%M:%S"));
        draw_text(&update_text, list_width + 15.0, screen_h - 10.0, 14.0, GRAY);

        next_frame().await
    }
}
