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
use opendp::traits::CollectionSize;
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
use crate::NoiseType::{Gaussian, Laplace};

mod noiser;
mod dataset;

const CSV_FILE_PATH: &'static str = "data/data.csv";

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(PartialEq)]
enum NoiseType {
    Laplace,
    Gaussian,
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
    let mut initial_accuracy = 0;
    let mut initial_alpha: f64 = 0.01;
    let mut aggregated_data = Vec::<u64>::new();
    let mut noised_data = Vec::<u64>::new();
    let mut active_noise_type = Laplace;

    // Noise the sensitive data - this is the first time we do this
    aggregate_data(noiser,
                   &aggregate_field,
                   accuracy_values[initial_accuracy],
                   initial_alpha,
                   &mut aggregated_data,
                   &mut noised_data);

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

    let menu_titles = vec!["Noise Type", "Increase Noise", "Decrease Noise", "Quit"];

    loop {
        terminal.draw(|rect| {
            draw_stuff(&aggregate_buckets,
                       initial_accuracy,
                       initial_alpha,
                       &mut aggregated_data,
                       &mut noised_data,
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
                    if active_noise_type == Laplace {
                        active_noise_type = Gaussian;
                    } else {
                        active_noise_type = Laplace;
                    }
                }
                KeyCode::Char('i') => {
                    initial_accuracy = (initial_accuracy + 1) % accuracy_values.size();
                    aggregate_data(noiser,
                                   &aggregate_field,
                                   accuracy_values[initial_accuracy],
                                   initial_alpha,
                                   &mut aggregated_data,
                                   &mut noised_data,
                    );
                }
                KeyCode::Char('d') => {
                    initial_accuracy = (initial_accuracy + accuracy_values.size() - 1) % accuracy_values.size();
                    aggregate_data(noiser,
                                   &aggregate_field,
                                   accuracy_values[initial_accuracy],
                                   initial_alpha,
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

fn draw_stuff(aggregate_buckets: &Vec<String>,
              current_accuracy: usize,
              alpha: f64,
              mut aggregated_data: &mut Vec<u64>,
              mut noised_data: &mut Vec<u64>,
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
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider(Span::raw("|"));

    rect.render_widget(tabs, header_chunks[0]);

    let noise_params = laplace_noise_params(current_accuracy, alpha);
    let noise_block = Paragraph::new(noise_params)
        .block(Block::default().title("Noise Params").borders(Borders::ALL))
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    rect.render_widget(noise_block, header_chunks[1]);

    draw_graphs(aggregate_buckets, aggregated_data, noised_data, rect, chunks);
}

fn draw_graphs(aggregate_buckets: &Vec<String>,
               aggregated_data: &mut Vec<u64>,
               noised_data: &mut Vec<u64>,
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

fn laplace_noise_params(current_accuracy: usize, alpha: f64) -> Vec<Spans<'static>> {
    vec![
        Spans::from(vec![
            Span::styled(format!("Accuracy: {}", current_accuracy), Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)),
        ]),
        Spans::from(vec![
            Span::styled(format!("Alpha: {}", alpha), Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)),
        ]),
    ]
}

fn aggregate_data(noiser: Noiser, aggregate_field: &String, accuracy: i64, alpha: f64, aggregated_data: &mut Vec<u64>, noised_data: &mut Vec<u64>) {
    aggregated_data.clear();
    noised_data.clear();
    aggregated_data.append(&mut noiser.aggregate_data(&aggregate_field).unwrap());
    let accuracy = accuracy;
    let theoretical_alpha = alpha;
    noised_data.append(&mut noiser.noised_data(&aggregated_data, accuracy, theoretical_alpha).unwrap())
}
