use std::{
    error::Error,
    fs,
    io
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    Terminal,
    widgets::{Block, Borders},
};
use tui::widgets::BarChart;

use crate::dataset::CsvDataSet;
use crate::noiser::{NoiseApplier, Noiser};

mod noiser;
mod dataset;

fn main() -> Result<(), Box<dyn Error>> {
    let contents = fs::read_to_string("data/data.csv")?;
    // Skip headers and then rejoin the CSV
    let contents = contents.split("\n").skip(1)
        .map(|x| x.to_string())
        .collect::<Vec<String>>().join("\n");

    let dataset = CsvDataSet {
        data: contents
    };
    let noiser = Noiser::new(&dataset);
    let aggregate_field = String::from("educ");
    let aggregated_data = noiser.aggregate_data(&aggregate_field).unwrap();
    let accuracy = 90;
    let theoretical_alpha = 0.000005;
    let noised_data = noiser.noised_data(&aggregated_data, accuracy, theoretical_alpha).unwrap();

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run_app(
        &mut terminal,
        &aggregated_data,
        &noised_data,
        &dataset.aggregate_buckets(&aggregate_field),
    );

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, res_sensitive: &Vec<u64>, res_noised: &Vec<u64>, categories: &Vec<String>) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, res_sensitive, res_noised, categories))?;

        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char('q') = key.code {
                return Ok(());
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, res_sensitive: &Vec<u64>, res_noised: &Vec<u64>, categories: &Vec<String>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());
    let block1 = Block::default().title("Sensitive Values").borders(Borders::ALL);
    let block2 = Block::default().title("Noised Values").borders(Borders::ALL);
    let mut chart_data1 = Vec::<(&str, u64)>::new();
    for (pos, e) in categories.iter().enumerate() {
        chart_data1.push((categories[pos].as_str(), res_sensitive[pos]) as (&str, u64))
    }
    let barchart = BarChart::default()
        .block(block1)
        .data(&chart_data1)
        .bar_width(9)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));
    f.render_widget(barchart, chunks[0]);

    let mut chart_data2 = Vec::<(&str, u64)>::new();
    for (pos, e) in categories.iter().enumerate() {
        chart_data2.push((categories[pos].as_str(), res_noised[pos]) as (&str, u64))
    }
    let barchart = BarChart::default()
        .block(block2)
        .data(&chart_data2)
        .bar_width(9)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));
    f.render_widget(barchart, chunks[1]);
}