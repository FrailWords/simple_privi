#![warn(unused_extern_crates)]
use std::error::Error;
use std::fs;
use std::io;
use std::io::Stdout;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use tui::{
    backend::CrosstermBackend, Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style}, Terminal,
    text::{Span, Spans},
    widgets::{
        Block, Borders, Paragraph, Tabs,
    }};
use tui::layout::Rect;
use tui::widgets::{BarChart, Wrap};

use crate::dataset::CsvDataSet;
use crate::noiser::{NoiseApplier, Noiser};

mod noiser;
mod dataset;

const CSV_FILE_PATH: &'static str = "data/data.csv";

enum Event<I> {
    Input(I),
    Tick,
}

fn main() -> Result<(), Box<dyn Error>> {
    let education_sensitive_field_to_aggregate: String = String::from("educ");
    let income_sensitive_field_to_aggregate: String = String::from("income");

    let contents = fs::read_to_string(CSV_FILE_PATH)?;
    // Skip headers and then rejoin the CSV
    let contents = contents.split("\n").skip(1)
        .map(|x| x.to_string())
        .collect::<Vec<String>>().join("\n");

    let dataset = CsvDataSet {
        data: &contents
    };
    let aggregate_field = &education_sensitive_field_to_aggregate;
    let mut noiser = Noiser::new(&dataset, aggregate_field);
    noiser.refresh_data();
    let aggregate_buckets = dataset.aggregate_buckets(aggregate_field);

    /*
    Start of UI related code
     */
    enable_raw_mode().expect("can run in raw mode");

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["Noise Type", "Increase Noise", "Decrease Noise", "Switch Field", "Quit"];

    loop {
        terminal.draw(|rect| {
            draw_stuff(&noiser,
                       &aggregate_buckets,
                       &menu_titles,
                       rect);
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                KeyCode::Char('n') => {
                    noiser.toggle_noise_type();
                }
                KeyCode::Char('i') => {
                    noiser.increase_noise();
                }
                KeyCode::Char('d') => {
                    noiser.decrease_noise();
                }
                KeyCode::Char('s') => {
                    match noiser.aggregate_field.as_str() {
                        "educ"=> {
                            noiser.aggregate_field = &income_sensitive_field_to_aggregate;
                        },
                        "income" => {
                            noiser.aggregate_field = &education_sensitive_field_to_aggregate;
                        },
                        _ => {}
                    }
                    noiser.accuracy = 0;
                    noiser.refresh_data();
                }
                _ => {}
            },
            Event::Tick => {}
        }
    }

    Ok(())
}

fn draw_stuff(noiser: &Noiser,
              aggregate_buckets: &Vec<String>,
              menu_titles: &Vec<&str>,
              rect: &mut Frame<CrosstermBackend<Stdout>>,
) {
    let size = rect.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(size);

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
        )
        .split(chunks[0]);

    let menu = menu_titles
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(
                    first, Style::default().add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled(rest, Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let tabs = Tabs::new(menu)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan))
        .divider(Span::raw("|"));

    rect.render_widget(tabs, header_chunks[0]);

    let noise_params = noise_params(noiser);
    let noise_block = Paragraph::new(noise_params)
        .block(Block::default().title("Noise Params").borders(Borders::ALL))
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    rect.render_widget(noise_block, header_chunks[1]);

    draw_graphs(aggregate_buckets, &noiser.aggregated_data, &noiser.noised_data, rect, chunks);
}

fn draw_graphs(aggregate_buckets: &Vec<String>,
               aggregated_data: &Vec<u64>,
               noised_data: &Vec<u64>,
               rect: &mut Frame<CrosstermBackend<Stdout>>,
               chunks: Vec<Rect>,
) {
    let graph_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
        )
        .split(chunks[1]);

    let block1 = Block::default().title("Sensitive Values").borders(Borders::ALL);
    let block2 = Block::default().title("Noised Values").borders(Borders::ALL);
    let mut chart_data1 = Vec::<(&str, u64)>::new();
    for (pos, _e) in aggregate_buckets.iter().enumerate() {
        chart_data1.push((aggregate_buckets[pos].as_str(), aggregated_data[pos]) as (&str, u64))
    }
    let left = BarChart::default()
        .block(block1)
        .data(&chart_data1)
        .bar_width(6)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    let mut chart_data2 = Vec::<(&str, u64)>::new();
    for (pos, _e) in aggregate_buckets.iter().enumerate() {
        chart_data2.push((aggregate_buckets[pos].as_str(), noised_data[pos]) as (&str, u64))
    }
    let right = BarChart::default()
        .block(block2)
        .data(&chart_data2)
        .bar_width(6)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    rect.render_widget(left, graph_chunks[0]);
    rect.render_widget(right, graph_chunks[1]);
}

fn noise_params(noiser: &Noiser) -> Vec<Spans<'static>> {
    vec![
        Spans::from(vec![
            Span::styled(format!("Type: {}", noiser.noise_type),
                         Style::default().fg(Color::Black)
                             .add_modifier(Modifier::BOLD)),
        ]),
        Spans::from(vec![
            Span::styled(format!("Noise: {}", noiser.accuracy),
                         Style::default().fg(Color::Black)
                             .add_modifier(Modifier::BOLD)),
        ]),
        Spans::from(vec![
            Span::styled(format!("Field: {}", noiser.aggregate_field),
                         Style::default().fg(Color::Black)
                             .add_modifier(Modifier::BOLD)),
        ]),
    ]
}
