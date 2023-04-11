use chrono::prelude::*;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use rand::{distributions::Alphanumeric, distributions::DistString, prelude::*};
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tui::{backend::CrosstermBackend, layout::{Alignment, Constraint, Direction, Layout}, style::{Color, Modifier, Style}, text::{Span, Spans}, widgets::{
    Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
}, Terminal, symbols, Frame};

use std::error::Error;
use std::fs;
use opendp::traits::CollectionSize;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::widgets::{BarChart, Dataset, GraphType, Wrap};
use crate::dataset::CsvDataSet;
use crate::noiser::{NoiseApplier, Noiser};

mod noiser;
mod dataset;

const CSV_FILE_PATH: &'static str = "data/data.csv";

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string(CSV_FILE_PATH)?;
    // Skip headers and then rejoin the CSV
    let contents = contents.split("\n").skip(1)
        .map(|x| x.to_string())
        .collect::<Vec<String>>().join("\n");

    let dataset = CsvDataSet {
        data: contents
    };
    let noiser = Noiser::new(&dataset);
    let aggregate_field = String::from("educ");
    let aggregate_buckets = dataset.aggregate_buckets(&aggregate_field);
    let accuracy_values: Vec<i64> = (1..=10).collect::<Vec<_>>();
    let mut current_accuracy = 0;
    let mut alpha: f64 = 0.01;
    let mut aggregated_data = Vec::<u64>::new();
    let mut noised_data = Vec::<u64>::new();
    aggregate_data(noiser,
                   &aggregate_field,
                   accuracy_values[current_accuracy],
                   alpha,
                   &mut aggregated_data,
                   &mut noised_data);

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

    let menu_titles = vec!["Home", "Increase Noise", "Decrease Noise", "Quit"];
    let mut active_menu_item = MenuItem::Home;

    loop {
        terminal.draw(|rect| {
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
                            first,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(rest, Style::default().fg(Color::DarkGray)),
                    ])
                })
                .collect();

            let tabs = Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            rect.render_widget(tabs, header_chunks[0]);

            let noise_params = vec![
                Spans::from(vec![
                    Span::styled(format!("Accuracy: {}", current_accuracy), Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)),
                ]),
                Spans::from(vec![
                    Span::styled(format!("Alpha: {}", alpha), Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)),
                ]),
            ];
            let noise_block = Paragraph::new(noise_params)
                .block(Block::default().title("Noise Params").borders(Borders::ALL))
                .style(Style::default().fg(Color::Green))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            rect.render_widget(noise_block, header_chunks[1]);

            let graph_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
                )
                .split(chunks[1]);

            let block1 = Block::default().title("Sensitive Values").borders(Borders::ALL);
            let block2 = Block::default().title("Noised Values").borders(Borders::ALL);
            let mut chart_data1 = Vec::<(&str, u64)>::new();
            for (pos, e) in aggregate_buckets.iter().enumerate() {
                chart_data1.push((aggregate_buckets[pos].as_str(), aggregated_data[pos]) as (&str, u64))
            }
            let left = BarChart::default()
                .block(block1)
                .data(&chart_data1)
                .bar_width(6)
                .bar_style(Style::default().fg(Color::Yellow))
                .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

            let mut chart_data2 = Vec::<(&str, u64)>::new();
            for (pos, e) in aggregate_buckets.iter().enumerate() {
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
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                KeyCode::Char('h') => active_menu_item = MenuItem::Home,
                KeyCode::Char('i') => {
                    current_accuracy = (current_accuracy + 1) % accuracy_values.size();
                    aggregate_data(noiser,
                                   &aggregate_field,
                                   accuracy_values[current_accuracy],
                                   alpha,
                                   &mut aggregated_data,
                                   &mut noised_data,
                    );
                }
                KeyCode::Char('d') => {
                    current_accuracy = (current_accuracy + accuracy_values.size() - 1) % accuracy_values.size();
                    aggregate_data(noiser,
                                   &aggregate_field,
                                   accuracy_values[current_accuracy],
                                   alpha,
                                   &mut aggregated_data,
                                   &mut noised_data,
                    );
                }
                _ => {}
            },
            Event::Tick => {}
        }
    }

    Ok(())
}

fn aggregate_data(noiser: Noiser, aggregate_field: &String, accuracy: i64, alpha: f64, aggregated_data: &mut Vec<u64>, noised_data: &mut Vec<u64>) {
    aggregated_data.clear();
    noised_data.clear();
    aggregated_data.append(&mut noiser.aggregate_data(&aggregate_field).unwrap());
    let accuracy = accuracy;
    let theoretical_alpha = alpha;
    noised_data.append(&mut noiser.noised_data(&aggregated_data, accuracy, theoretical_alpha).unwrap())
}
