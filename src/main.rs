mod csv_reader;

use std::{fs, process};
use opendp::accuracy::accuracy_to_discrete_laplacian_scale;
use opendp::domains::{AllDomain, VectorDomain};
use opendp::error::{ExplainUnwrap, Fallible};
use opendp::transformations::{make_b_ary_tree, make_count_by_categories, make_select_column, make_sized_bounded_mean, make_split_dataframe};
use opendp::measurements::make_base_discrete_laplace;
use opendp::metrics::{L2Distance, L1Distance, IntDistance, SymmetricDistance};
use opendp::traits::{Hashable, Number};

use csv_reader::read_data;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use std::fs::File;
use std::io::{BufReader, Read};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
    Frame, Terminal,
};
use tui::widgets::BarChart;

fn main() -> Result<(), Box<dyn Error>>  {
    let accuracy = 25;
    let theoretical_alpha = 0.5;
    //* `scale` - Noise scale parameter for the laplace distribution. `scale` == sqrt(2) * standard_deviation.
    let scale = accuracy_to_discrete_laplacian_scale(accuracy as f64, theoretical_alpha)?;
    println!("scale: {scale}");
    let base_dl = make_base_discrete_laplace::<AllDomain<i8>, f64>(scale)?;
    let result = base_dl.invoke(&0);
    println!("{}", result.unwrap());

    let categories = (1u8..21).map(|x| x.to_string()).collect::<Vec<_>>();
    let col_names = Vec::from(["age", "sex", "educ", "race", "income", "married"]);

    let df_transformer = make_split_dataframe(Option::from(","), col_names)?;
    let select_col = make_select_column::<_, String>("educ")?;
    let count_by_education = make_count_by_categories::<L2Distance<u8>, String, u64>(categories.clone(), true).unwrap();
    let d = "59,1,9,1,0,1\n31,0,1,3,17000,0\n".to_owned();
    let contents = fs::read_to_string("data/data.csv")?;
    let mut contents = contents.split("\n").skip(1)
        .map(|x| x.to_string())
        .collect::<Vec<String>>().join("\n");
    let chain = (df_transformer >> select_col >> count_by_education)?;
    let res_sensitive = chain.invoke(&contents)?;
    println!("{:?}", res_sensitive);

    let discrete_lp = make_base_discrete_laplace::<VectorDomain<AllDomain<u64>>, _>(
        scale
    )?;
    let res_noised = discrete_lp.invoke(&res_sensitive)?;
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run_app(&mut terminal, &res_sensitive, &res_noised, &categories);

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