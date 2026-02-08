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

    // Precalcolati per evitare fold ogni frame
    current_min: f32,
    current_max: f32,
    h24_min: f32,
    h24_max: f32,
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

#[derive(Debug)]
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

const MAX_SELECTED: usize = 8;

// ────────────────────────────────────────────────
// Fetch functions (invariate)
// ────────────────────────────────────────────────

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

            let change_percent = if prev_close != 0.0 {
                ((current_price - prev_close) / prev_close) * 100.0
            } else {
                0.0
            };

            return Ok((current_price, change_percent));
        }
    }

    Err("Failed to parse stock data".into())
}

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
            if let (Some(timestamp), Some(price_val)) = (ts.as_i64(), closes.get(i)) {
                if let Some(price) = price_val.as_f64() {
                    let dt = DateTime::from_timestamp(timestamp, 0).unwrap_or(Utc::now());
                    times.push(dt);
                    prices.push(price as f32);
                }
            }
        }

        return Ok((prices, times));
    }

    Err("Failed to parse historical data".into())
}

// ────────────────────────────────────────────────
// Scrollbar (leggermente ottimizzata)
// ────────────────────────────────────────────────

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

    draw_rectangle(
        scrollbar_x,
        scrollbar_y,
        scrollbar_width,
        height,
        Color::from_rgba(20, 20, 25, 255),
    );
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

    let thumb_height = ((visible_height / total_content_height) * height)
        .max(30.0)
        .min(height - 10.0);
    let scroll_range = height - thumb_height;
    let thumb_y = scrollbar_y + (scroll_offset / max_scroll) * scroll_range;

    let (mx, my) = mouse_position();
    let is_on_scrollbar = mx >= scrollbar_x
        && mx <= scrollbar_x + scrollbar_width
        && my >= scrollbar_y
        && my <= scrollbar_y + height;
    let is_on_thumb = is_on_scrollbar && my >= thumb_y && my <= thumb_y + thumb_height;

    let thumb_color = if scrollbar_state.dragging {
        Color::from_rgba(120, 120, 140, 255)
    } else if is_on_thumb {
        Color::from_rgba(100, 100, 120, 255)
    } else if is_on_scrollbar {
        Color::from_rgba(80, 80, 100, 255)
    } else {
        Color::from_rgba(60, 60, 80, 255)
    };

    draw_rectangle(
        scrollbar_x + 2.0,
        thumb_y + 2.0,
        scrollbar_width - 4.0,
        thumb_height - 4.0,
        thumb_color,
    );

    // tre linee sul thumb
    let indicator_y = thumb_y + thumb_height / 2.0;
    for i in 0..3 {
        let ly = indicator_y - 2.0 + (i as f32 * 2.0);
        draw_line(
            scrollbar_x + 4.0,
            ly,
            scrollbar_x + scrollbar_width - 4.0,
            ly,
            1.0,
            Color::from_rgba(40, 40, 50, 200),
        );
    }

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
        let new_scroll = (scrollbar_state.drag_start_scroll + scroll_delta).clamp(0.0, max_scroll);
        return new_scroll;
    }

    // click sulla track
    if is_mouse_button_pressed(MouseButton::Left) && is_on_scrollbar && !is_on_thumb {
        let click_ratio = (my - scrollbar_y) / height;
        let new_scroll = (click_ratio * max_scroll).clamp(0.0, max_scroll);
        return new_scroll;
    }

    scroll_offset
}

// ────────────────────────────────────────────────
// Item lista con checkbox (invariato, ma chiamato meno volte)
// ────────────────────────────────────────────────

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
    let cs = 20.0;
    let cx = x + 10.0;
    let cy = y + (height - cs) / 2.0;

    let checkbox_color = if is_selected {
        Color::from_rgba(50, 100, 200, 255)
    } else {
        Color::from_rgba(40, 40, 50, 255)
    };
    draw_rectangle(cx, cy, cs, cs, checkbox_color);
    draw_rectangle_lines(cx, cy, cs, cs, 2.0, Color::from_rgba(100, 100, 120, 255));

    if is_selected {
        draw_line(cx + 4.0, cy + 10.0, cx + 8.0, cy + 16.0, 2.0, WHITE);
        draw_line(cx + 8.0, cy + 16.0, cx + 16.0, cy + 4.0, 2.0, WHITE);
    }

    draw_text(&stock.symbol, x + 45.0, y + 22.0, 20.0, WHITE);

    if stock.current_price > 0.0 {
        draw_text(
            &format!("${:.2}", stock.current_price),
            x + 45.0,
            y + 42.0,
            20.0,
            LIGHTGRAY,
        );

        let ch_color = if stock.change_percent >= 0.0 {
            Color::from_rgba(0, 200, 100, 255)
        } else {
            Color::from_rgba(220, 50, 50, 255)
        };
        draw_text(
            &format!("{:+.2}%", stock.change_percent),
            x + 45.0,
            y + 60.0,
            16.0,
            ch_color,
        );

        if stock.change_24h_percent != 0.0 {
            draw_text(
                &format!("24h: {:+.2}%", stock.change_24h_percent),
                x + width - 90.0,
                y + 60.0,
                16.0,
                Color::from_rgba(100, 150, 255, 255),
            );
        }
    } else {
        draw_text("Caricamento...", x + 45.0, y + 48.0, 16.0, GRAY);
    }

    is_clicked
}

// ────────────────────────────────────────────────
// Pannello lista sinistra – SOLO ITEM VISIBILI
// ────────────────────────────────────────────────

fn draw_list_panel(
    stocks: &HashMap<String, StockData>,
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
    draw_text(
        &format!("Selezionati: {}", selected_symbols.len()),
        x + width - 130.0,
        y + 30.0,
        18.0,
        LIGHTGRAY,
    );

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
    let item_total_h = item_height + item_padding;
    let start_y = y + 50.0;
    let visible_h = height - 50.0;
    let mouse_pos = mouse_position();

    // Solo items visibili
    let first_idx = (scroll_offset / item_total_h).floor().max(0.0) as usize;
    let last_idx =
        (((scroll_offset + visible_h) / item_total_h).ceil() as usize).min(symbols.len());

    let mut clicked_symbol = None;
    let mut new_scroll = scroll_offset;

    // Mouse wheel
    let (_wx, wy) = mouse_wheel();
    let mouse_in_list = mouse_pos.0 >= x
        && mouse_pos.0 <= x + width
        && mouse_pos.1 >= y
        && mouse_pos.1 <= y + height;
    if mouse_in_list && !scrollbar_state.dragging {
        new_scroll -= wy * 60.0; // invertito per feeling naturale
        let max_scroll = (symbols.len() as f32 * item_total_h - visible_h).max(0.0);
        new_scroll = new_scroll.clamp(0.0, max_scroll);
    }

    let items_width = width - 30.0;

    for i in first_idx..last_idx {
        let symbol = &symbols[i];
        if let Some(stock) = stocks.get(symbol) {
            let item_y = start_y + (i as f32 * item_total_h) - new_scroll;

            if draw_list_item(
                stock,
                x + 10.0,
                item_y,
                items_width - 20.0,
                item_height,
                selected_symbols.contains(symbol),
                mouse_pos,
            ) {
                clicked_symbol = Some(symbol.clone());
            }
        }
    }

    // Scrollbar
    let total_content_h = symbols.len() as f32 * item_total_h;
    new_scroll = draw_scrollbar(
        x,
        start_y,
        width,
        visible_h,
        new_scroll,
        total_content_h,
        visible_h,
        scrollbar_state,
    );

    (clicked_symbol, new_scroll)
}

// ────────────────────────────────────────────────
// Mini-grafico – usa valori precalcolati
// ────────────────────────────────────────────────

fn draw_mini_chart(stock: &StockData, x: f32, y: f32, width: f32, height: f32) {
    draw_rectangle(x, y, width, height, Color::from_rgba(30, 30, 40, 255));
    draw_rectangle_lines(x, y, width, height, 2.0, Color::from_rgba(50, 50, 60, 255));

    draw_text(&stock.symbol, x + 15.0, y + 28.0, 24.0, WHITE);
    draw_text(
        &format!("${:.2}", stock.current_price),
        x + 15.0,
        y + 52.0,
        20.0,
        LIGHTGRAY,
    );

    let ch_color = if stock.change_percent >= 0.0 {
        Color::from_rgba(0, 200, 100, 255)
    } else {
        Color::from_rgba(220, 50, 50, 255)
    };
    draw_text(
        &format!("{:+.2}%", stock.change_percent),
        x + width - 90.0,
        y + 32.0,
        20.0,
        ch_color,
    );

    if stock.change_24h_percent != 0.0 {
        draw_text(
            &format!("24h: {:+.2}%", stock.change_24h_percent),
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

    let chart_x = x + 70.0;
    let chart_y = y + 75.0;
    let chart_w = width - 90.0;
    let chart_h = height - 105.0;

    // ── Layer 24h (blu) ───────────────────────────────────────
    if stock.prices_24h_ago.len() >= 2 && stock.h24_max > stock.h24_min {
        let range = stock.h24_max - stock.h24_min;
        let padding = range * 0.08;
        let min_val = stock.h24_min - padding;
        let max_val = stock.h24_max + padding;
        let val_range = max_val - min_val;

        let blue = Color::from_rgba(100, 150, 255, 150);
        let mut fill_blue = blue;
        fill_blue.a = 0.1;

        for i in 0..stock.prices_24h_ago.len() - 1 {
            let x1 = chart_x + (i as f32 / (stock.prices_24h_ago.len() - 1) as f32) * chart_w;
            let y1 =
                chart_y + chart_h - ((stock.prices_24h_ago[i] - min_val) / val_range) * chart_h;
            let x2 = chart_x + ((i + 1) as f32 / (stock.prices_24h_ago.len() - 1) as f32) * chart_w;
            let y2 =
                chart_y + chart_h - ((stock.prices_24h_ago[i + 1] - min_val) / val_range) * chart_h;

            draw_triangle(
                vec2(x1, y1),
                vec2(x2, y2),
                vec2(x2, chart_y + chart_h),
                fill_blue,
            );
            draw_triangle(
                vec2(x1, y1),
                vec2(x1, chart_y + chart_h),
                vec2(x2, chart_y + chart_h),
                fill_blue,
            );
            draw_line(x1, y1, x2, y2, 2.2, blue);
        }

        // etichette sinistra (blu)
        for i in 0..3 {
            let frac = i as f32 / 2.0;
            let price = max_val - frac * val_range;
            let gy = chart_y + frac * chart_h;
            draw_text(
                &format!("${:.2}", price),
                x + 5.0,
                gy + 5.0,
                13.0,
                Color::from_rgba(100, 150, 255, 180),
            );
        }
    }

    // ── Layer corrente (verde/rosso) ───────────────────────────
    if stock.current_max > stock.current_min {
        let range = stock.current_max - stock.current_min;
        let padding = range * 0.08;
        let min_val = stock.current_min - padding;
        let max_val = stock.current_max + padding;
        let val_range = max_val - min_val;

        let line_color = if stock.change_percent >= 0.0 {
            Color::from_rgba(0, 220, 120, 255)
        } else {
            Color::from_rgba(255, 80, 80, 255)
        };
        let mut fill_color = line_color;
        fill_color.a = 0.25;

        for i in 0..stock.prices.len() - 1 {
            let x1 = chart_x + (i as f32 / (stock.prices.len() - 1) as f32) * chart_w;
            let y1 = chart_y + chart_h - ((stock.prices[i] - min_val) / val_range) * chart_h;
            let x2 = chart_x + ((i + 1) as f32 / (stock.prices.len() - 1) as f32) * chart_w;
            let y2 = chart_y + chart_h - ((stock.prices[i + 1] - min_val) / val_range) * chart_h;

            draw_triangle(
                vec2(x1, y1),
                vec2(x2, y2),
                vec2(x2, chart_y + chart_h),
                fill_color,
            );
            draw_triangle(
                vec2(x1, y1),
                vec2(x1, chart_y + chart_h),
                vec2(x2, chart_y + chart_h),
                fill_color,
            );
            draw_line(x1, y1, x2, y2, 3.0, line_color);
        }

        // etichette destra
        for i in 0..5 {
            let frac = i as f32 / 4.0;
            let price = max_val - frac * val_range;
            let gy = chart_y + frac * chart_h;

            let txt = format!("${:.2}", price);
            let tw = 45.0;
            draw_rectangle(
                x + width - tw - 8.0,
                gy - 8.0,
                tw,
                14.0,
                Color::from_rgba(30, 30, 40, 200),
            );
            draw_text(&txt, x + width - tw - 5.0, gy + 4.0, 13.0, line_color);
        }

        // Legenda
        let ly = y + height - 10.0;
        draw_rectangle(
            x + 8.0,
            ly - 22.0,
            width - 16.0,
            24.0,
            Color::from_rgba(20, 20, 25, 220),
        );
        draw_rectangle_lines(
            x + 8.0,
            ly - 22.0,
            width - 16.0,
            24.0,
            1.0,
            Color::from_rgba(50, 50, 60, 255),
        );

        draw_line(x + 12.0, ly - 10.0, x + 30.0, ly - 10.0, 3.0, line_color);
        draw_text(
            &format!("Ora (${:.2} – {:.2})", stock.current_min, stock.current_max),
            x + 35.0,
            ly - 5.0,
            15.0,
            WHITE,
        );

        let midx = x + width / 2.0 + 10.0;
        draw_line(
            midx,
            ly - 10.0,
            midx + 18.0,
            ly - 10.0,
            2.2,
            Color::from_rgba(100, 150, 255, 255),
        );

        if stock.prices_24h_ago.len() >= 2 {
            draw_text(
                &format!("24h (${:.2} – {:.2})", stock.h24_min, stock.h24_max),
                midx + 23.0,
                ly - 5.0,
                15.0,
                Color::from_rgba(150, 170, 200, 255),
            );
        } else {
            draw_text(
                "24h",
                midx + 23.0,
                ly - 5.0,
                15.0,
                Color::from_rgba(150, 170, 200, 255),
            );
        }
    }
}

// ────────────────────────────────────────────────
// Pannello grafici – con limite altezza
// ────────────────────────────────────────────────

fn draw_charts_panel(
    stocks: &HashMap<String, StockData>,
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

    let selected: Vec<&String> = selected_symbols.iter().collect();
    let count = selected.len();

    let cols = if count == 1 {
        1
    } else if count <= 4 {
        2
    } else {
        3
    };
    let rows = (count + cols - 1) / cols;

    let padding = 15.0;
    let max_chart_h = 500.0;

    let avail_w = (width - padding * (cols as f32 + 1.0)) / cols as f32;
    let avail_h = (height - padding * (rows as f32 + 1.0)) / rows as f32;

    let chart_h = avail_h.min(max_chart_h);
    let chart_w = avail_w;

    let total_charts_h = rows as f32 * chart_h + padding * (rows as f32 + 1.0);
    let v_offset = if total_charts_h < height {
        (height - total_charts_h) / 2.0
    } else {
        0.0
    };

    for (i, &symbol) in selected.iter().enumerate() {
        if let Some(stock) = stocks.get(symbol) {
            let col = i % cols;
            let row = i / cols;

            let cx = x + padding + col as f32 * (chart_w + padding);
            let cy = y + padding + v_offset + row as f32 * (chart_h + padding);

            draw_mini_chart(stock, cx, cy, chart_w, chart_h);
        }
    }
}

// ────────────────────────────────────────────────
// Worker di aggiornamento (invariati)
// ────────────────────────────────────────────────

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
                if let Ok((price, change)) = fetch_stock_data(symbol) {
                    if let Ok(mut lock) = stocks.lock() {
                        if let Some(s) = lock.get_mut(symbol) {
                            s.prices.push(price);
                            s.timestamps.push(now);
                            if s.prices.len() > 60 {
                                s.prices.remove(0);
                                s.timestamps.remove(0);
                            }
                            s.current_price = price;
                            s.change_percent = change;

                            // aggiorna min/max
                            if !s.prices.is_empty() {
                                s.current_min =
                                    s.prices.iter().cloned().fold(f32::INFINITY, f32::min);
                                s.current_max =
                                    s.prices.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                            }
                        }
                    }
                }
            }

            if let Ok(mut lu) = last_update.lock() {
                *lu = now;
            }
        }
    });
}

fn start_24h_update_worker(stocks: Arc<Mutex<HashMap<String, StockData>>>, symbols: Vec<String>) {
    thread::spawn(move || {
        loop {
            for symbol in &symbols {
                if let Ok((prices, timestamps)) = fetch_24h_historical_data(symbol) {
                    if let Ok(mut lock) = stocks.lock() {
                        if let Some(s) = lock.get_mut(symbol) {
                            s.prices_24h_ago = prices.clone();
                            s.timestamps_24h_ago = timestamps;

                            if !prices.is_empty() {
                                s.h24_min = prices.iter().cloned().fold(f32::INFINITY, f32::min);
                                s.h24_max =
                                    prices.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

                                if s.current_price > 0.0 && !prices.is_empty() {
                                    let p24 = prices[0];
                                    if p24 > 0.0 {
                                        s.change_24h_percent =
                                            ((s.current_price - p24) / p24) * 100.0;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(300));
        }
    });
}

fn initial_fetch(stocks: Arc<Mutex<HashMap<String, StockData>>>, symbols: Vec<String>) {
    let stocks_c = stocks.clone();
    let value = symbols.clone();
    thread::spawn(move || {
        for sym in &value {
            if let Ok((price, ch)) = fetch_stock_data(sym) {
                if let Ok(mut lock) = stocks_c.lock() {
                    if let Some(s) = lock.get_mut(sym) {
                        s.current_price = price;
                        s.change_percent = ch;
                        s.prices.push(price);
                        s.timestamps.push(Utc::now());

                        if !s.prices.is_empty() {
                            s.current_min = s.prices.iter().cloned().fold(f32::INFINITY, f32::min);
                            s.current_max =
                                s.prices.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                        }
                    }
                }
            }
        }
    });

    let stocks_c2 = stocks.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(3));
        for sym in &symbols {
            if let Ok((prices, ts)) = fetch_24h_historical_data(sym) {
                if let Ok(mut lock) = stocks_c2.lock() {
                    if let Some(s) = lock.get_mut(sym) {
                        s.prices_24h_ago = prices.clone();
                        s.timestamps_24h_ago = ts;

                        if !prices.is_empty() {
                            s.h24_min = prices.iter().cloned().fold(f32::INFINITY, f32::min);
                            s.h24_max = prices.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

                            if s.current_price > 0.0 {
                                let p24 = prices[0];
                                if p24 > 0.0 {
                                    s.change_24h_percent = ((s.current_price - p24) / p24) * 100.0;
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

#[macroquad::main("Stock Tracker – Ottimizzato")]
async fn main() {
    let mut app = App::new();
    let mut scroll_offset = 0.0f32;

    let symbols = vec![
        "SPY", "QQQ", "DIA", "IWM", "AAPL", "MSFT", "GOOGL", "AMZN", "TSLA", "NVDA", "META",
        "NFLX", "AMD", "INTC", "JPM", "BAC", "WMT", "V", "MA", "XOM", "BTC-USD", "ETH-USD",
        "BNB-USD", "XRP-USD", "ADA-USD", "SOL-USD", "DOGE-USD",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();

    {
        let mut stocks = app.stocks.lock().unwrap();
        for sym in &symbols {
            stocks.insert(
                sym.clone(),
                StockData {
                    symbol: sym.clone(),
                    prices: vec![],
                    timestamps: vec![],
                    prices_24h_ago: vec![],
                    timestamps_24h_ago: vec![],
                    current_price: 0.0,
                    change_percent: 0.0,
                    change_24h_percent: 0.0,
                    current_min: f32::INFINITY,
                    current_max: f32::NEG_INFINITY,
                    h24_min: f32::INFINITY,
                    h24_max: f32::NEG_INFINITY,
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
        let list_w = 320.0;
        let charts_w = screen_w - list_w;

        let stocks_guard = app.stocks.lock().unwrap();

        let (clicked, new_scroll) = draw_list_panel(
            &stocks_guard,
            &symbols,
            &app.selected_symbols,
            0.0,
            0.0,
            list_w,
            screen_h,
            scroll_offset,
            &mut app.scrollbar_state,
        );

        scroll_offset = new_scroll;

        if let Some(sym) = clicked {
            if app.selected_symbols.contains(&sym) {
                app.selected_symbols.remove(&sym);
            } else if app.selected_symbols.len() < MAX_SELECTED {
                app.selected_symbols.insert(sym);
            }
            // else → potresti aggiungere un messaggio "Massimo raggiunto"
        }

        draw_line(
            list_w,
            0.0,
            list_w,
            screen_h,
            2.0,
            Color::from_rgba(50, 50, 60, 255),
        );

        draw_charts_panel(
            &stocks_guard,
            &app.selected_symbols,
            list_w,
            0.0,
            charts_w,
            screen_h,
        );

        let last_up = *app.last_update.lock().unwrap();
        draw_text(
            &format!("Aggiornamento: {}", last_up.format("%H:%M:%S")),
            list_w + 15.0,
            screen_h - 10.0,
            14.0,
            GRAY,
        );

        drop(stocks_guard); // esplicito, ma non strettamente necessario

        next_frame().await;
    }
}
