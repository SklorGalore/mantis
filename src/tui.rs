use std::fs;
use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{prelude::*, widgets::*};

use crate::case::Network;
use crate::parse;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Buses,
    Branches,
    Generators,
    Loads,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Buses, Tab::Branches, Tab::Generators, Tab::Loads];

    fn label(&self) -> &'static str {
        match self {
            Tab::Buses => "Buses",
            Tab::Branches => "Branches",
            Tab::Generators => "Generators",
            Tab::Loads => "Loads",
        }
    }

    fn index(&self) -> usize {
        Self::ALL.iter().position(|t| t == self).unwrap()
    }

    fn next(&self) -> Tab {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }

    fn prev(&self) -> Tab {
        let i = (self.index() + Self::ALL.len() - 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    FileBrowser,
    Editing,
    BusDisplay,
}

struct EditState {
    row: usize,
    col: usize,
    buf: String,
    col_count: usize,
}

pub struct App {
    network: Option<Network>,
    active_tab: Tab,
    table_state: TableState,
    mode: Mode,
    file_list: Vec<String>,
    file_list_state: ListState,
    editing: Option<EditState>,
    status_msg: String,
    bus_display_scroll: u16,
}

impl App {
    fn new() -> Self {
        Self {
            network: None,
            active_tab: Tab::Buses,
            table_state: TableState::default(),
            mode: Mode::Normal,
            file_list: Vec::new(),
            file_list_state: ListState::default(),
            editing: None,
            status_msg: String::from("Press 'o' to open a case file"),
            bus_display_scroll: 0,
        }
    }

    fn row_count(&self) -> usize {
        let net = match &self.network {
            Some(n) => n,
            None => return 0,
        };
        match self.active_tab {
            Tab::Buses => net.buses.len(),
            Tab::Branches => net.branches.len(),
            Tab::Generators => net.generators.len(),
            Tab::Loads => net.loads.len(),
        }
    }

    fn scan_case_files(&mut self) {
        self.file_list.clear();
        if let Ok(entries) = fs::read_dir("cases") {
            let mut files: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().ends_with(".raw") {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect();
            files.sort();
            self.file_list = files;
        }
    }

    fn load_case(&mut self, filename: &str) {
        let path = format!("cases/{}", filename);
        let mut net = parse::read_case_v33(&path);
        net.rebuild_bus_map();
        self.status_msg = format!(
            "Loaded: {} ({} buses, {} branches)",
            filename,
            net.buses.len(),
            net.branches.len()
        );
        self.network = Some(net);
        self.table_state = TableState::default();
        if self.row_count() > 0 {
            self.table_state.select(Some(0));
        }
    }

    fn run_dc(&mut self) {
        if let Some(net) = &mut self.network {
            if net.dc_approximation() {
                self.status_msg = format!(
                    "DC solved: {} buses, {} branches with flows",
                    net.buses.len(),
                    net.branches.len()
                );
            } else {
                self.status_msg = String::from("DC load flow failed -- check network data");
            }
        } else {
            self.status_msg = String::from("No network loaded");
        }
    }

    fn toggle_status(&mut self) {
        let row = match self.table_state.selected() {
            Some(r) => r,
            None => return,
        };
        let net = match &mut self.network {
            Some(n) => n,
            None => return,
        };
        match self.active_tab {
            Tab::Buses => {
                if row >= net.buses.len() {
                    return;
                }
                let b = &mut net.buses[row];
                b.bus_status = !b.bus_status;
                if b.bus_status {
                    if b.bus_type == crate::case::BusType::OUT {
                        b.bus_type = crate::case::BusType::PQ;
                    }
                    self.status_msg = format!("Bus {} in service", b.bus_id);
                } else {
                    b.bus_type = crate::case::BusType::OUT;
                    self.status_msg = format!("Bus {} out of service", b.bus_id);
                }
            }
            Tab::Branches => {
                if row >= net.branches.len() {
                    return;
                }
                let br = &mut net.branches[row];
                br.branch_status = !br.branch_status;
                self.status_msg = format!(
                    "Branch {} {}",
                    br.id,
                    if br.branch_status {
                        "in service"
                    } else {
                        "out of service"
                    }
                );
            }
            _ => {
                self.status_msg = String::from("Toggle only applies to buses and branches");
            }
        }
    }

    fn start_edit(&mut self) {
        if self.network.is_none() {
            return;
        }
        let row = match self.table_state.selected() {
            Some(r) => r,
            None => return,
        };
        let col_count = match self.active_tab {
            Tab::Buses => 9,
            Tab::Branches => 9,
            Tab::Generators => 9,
            Tab::Loads => 5,
        };
        let val = self.cell_value(row, 0);
        self.editing = Some(EditState {
            row,
            col: 0,
            buf: val,
            col_count,
        });
        self.mode = Mode::Editing;
        self.status_msg = String::from(
            "Editing: Left/Right to select field, type to modify, Enter to confirm, Esc to cancel",
        );
    }

    fn cell_value(&self, row: usize, col: usize) -> String {
        let net = match &self.network {
            Some(n) => n,
            None => return String::new(),
        };
        match self.active_tab {
            Tab::Buses => {
                if row >= net.buses.len() {
                    return String::new();
                }
                let b = &net.buses[row];
                match col {
                    0 => b.bus_id.to_string(),
                    1 => b.bus_name.clone(),
                    2 => format!("{}", b.bus_type),
                    3 => format!("{:.6}", b.voltage),
                    4 => format!("{:.4}", b.angle),
                    5 => format!("{:.1}", b.nom_voltage),
                    6 => {
                        let (p, _) = net.bus_mismatch(b.bus_id);
                        format!("{:.2}", p)
                    }
                    7 => {
                        let (_, q) = net.bus_mismatch(b.bus_id);
                        format!("{:.2}", q)
                    }
                    8 => {
                        if b.bus_status {
                            "1".into()
                        } else {
                            "0".into()
                        }
                    }
                    _ => String::new(),
                }
            }
            Tab::Branches => {
                if row >= net.branches.len() {
                    return String::new();
                }
                let br = &net.branches[row];
                match col {
                    0 => br.id.to_string(),
                    1 => br.from_bus.to_string(),
                    2 => br.to_bus.to_string(),
                    3 => format!("{}", br.branch_type),
                    4 => format!("{:.6}", br.resistance),
                    5 => format!("{:.6}", br.reactance),
                    6 => format!("{:.1}", br.operating_limit),
                    7 => format!("{:.3}", br.flow),
                    8 => {
                        if br.branch_status {
                            "1".into()
                        } else {
                            "0".into()
                        }
                    }
                    _ => String::new(),
                }
            }
            Tab::Generators => {
                if row >= net.generators.len() {
                    return String::new();
                }
                let g = &net.generators[row];
                match col {
                    0 => g.gen_id.to_string(),
                    1 => g.gen_bus_id.to_string(),
                    2 => g.gen_name.clone(),
                    3 => format!("{:.3}", g.p_gen),
                    4 => format!("{:.3}", g.q_gen),
                    5 => format!("{:.5}", g.v_setpoint),
                    6 => format!("{:.3}", g.p_min),
                    7 => format!("{:.3}", g.p_max),
                    8 => {
                        if g.gen_status {
                            "1".into()
                        } else {
                            "0".into()
                        }
                    }
                    _ => String::new(),
                }
            }
            Tab::Loads => {
                if row >= net.loads.len() {
                    return String::new();
                }
                let l = &net.loads[row];
                match col {
                    0 => l.load_id.to_string(),
                    1 => l.bus_id.to_string(),
                    2 => l.load_name.clone(),
                    3 => format!("{:.3}", l.real_load),
                    4 => format!("{:.3}", l.imag_load),
                    _ => String::new(),
                }
            }
        }
    }

    fn apply_edit(&mut self) {
        let edit = match &self.editing {
            Some(e) => e,
            None => return,
        };
        let row = edit.row;
        let col = edit.col;
        let val = edit.buf.trim().to_string();

        let net = match &mut self.network {
            Some(n) => n,
            None => return,
        };

        match self.active_tab {
            Tab::Buses => {
                if row >= net.buses.len() {
                    return;
                }
                let b = &mut net.buses[row];
                match col {
                    1 => b.bus_name = val,
                    3 => {
                        if let Ok(v) = val.parse::<f32>() {
                            b.voltage = v;
                        }
                    }
                    4 => {
                        if let Ok(v) = val.parse::<f32>() {
                            b.angle = v;
                        }
                    }
                    5 => {
                        if let Ok(v) = val.parse::<f32>() {
                            b.nom_voltage = v;
                        }
                    }
                    8 => {
                        let in_service = val != "0";
                        b.bus_status = in_service;
                        if !in_service {
                            b.bus_type = crate::case::BusType::OUT;
                        } else if b.bus_type == crate::case::BusType::OUT {
                            b.bus_type = crate::case::BusType::PQ;
                        }
                    }
                    _ => {} // ID, Type, P mis, Q mis not editable
                }
            }
            Tab::Branches => {
                if row >= net.branches.len() {
                    return;
                }
                let br = &mut net.branches[row];
                match col {
                    1 => {
                        if let Ok(v) = val.parse::<usize>() {
                            br.from_bus = v;
                        }
                    }
                    2 => {
                        if let Ok(v) = val.parse::<usize>() {
                            br.to_bus = v;
                        }
                    }
                    4 => {
                        if let Ok(v) = val.parse::<f32>() {
                            br.resistance = v;
                        }
                    }
                    5 => {
                        if let Ok(v) = val.parse::<f32>() {
                            br.reactance = v;
                        }
                    }
                    6 => {
                        if let Ok(v) = val.parse::<f32>() {
                            br.operating_limit = v;
                        }
                    }
                    8 => br.branch_status = val != "0",
                    _ => {}
                }
            }
            Tab::Generators => {
                if row >= net.generators.len() {
                    return;
                }
                let g = &mut net.generators[row];
                match col {
                    1 => {
                        if let Ok(v) = val.parse::<usize>() {
                            g.gen_bus_id = v;
                        }
                    }
                    2 => g.gen_name = val,
                    3 => {
                        if let Ok(v) = val.parse::<f32>() {
                            g.p_gen = v;
                        }
                    }
                    4 => {
                        if let Ok(v) = val.parse::<f32>() {
                            g.q_gen = v;
                        }
                    }
                    5 => {
                        if let Ok(v) = val.parse::<f32>() {
                            g.v_setpoint = v;
                        }
                    }
                    6 => {
                        if let Ok(v) = val.parse::<f32>() {
                            g.p_min = v;
                        }
                    }
                    7 => {
                        if let Ok(v) = val.parse::<f32>() {
                            g.p_max = v;
                        }
                    }
                    8 => g.gen_status = val != "0",
                    _ => {}
                }
            }
            Tab::Loads => {
                if row >= net.loads.len() {
                    return;
                }
                let l = &mut net.loads[row];
                match col {
                    1 => {
                        if let Ok(v) = val.parse::<usize>() {
                            l.bus_id = v;
                        }
                    }
                    2 => l.load_name = val,
                    3 => {
                        if let Ok(v) = val.parse::<f32>() {
                            l.real_load = v;
                        }
                    }
                    4 => {
                        if let Ok(v) = val.parse::<f32>() {
                            l.imag_load = v;
                        }
                    }
                    _ => {}
                }
            }
        }

        self.status_msg = String::from("Edit applied");
    }

    fn col_label(&self, col: usize) -> &'static str {
        match self.active_tab {
            Tab::Buses => match col {
                0 => "ID",
                1 => "Name",
                2 => "Type",
                3 => "Voltage",
                4 => "Angle",
                5 => "Nom kV",
                6 => "P mis",
                7 => "Q mis",
                8 => "Status",
                _ => "",
            },
            Tab::Branches => match col {
                0 => "ID",
                1 => "From",
                2 => "To",
                3 => "Type",
                4 => "R",
                5 => "X",
                6 => "RateA",
                7 => "Flow",
                8 => "Status",
                _ => "",
            },
            Tab::Generators => match col {
                0 => "ID",
                1 => "Bus",
                2 => "Name",
                3 => "P",
                4 => "Q",
                5 => "Vset",
                6 => "Pmin",
                7 => "Pmax",
                8 => "Status",
                _ => "",
            },
            Tab::Loads => match col {
                0 => "ID",
                1 => "Bus",
                2 => "Name",
                3 => "P",
                4 => "Q",
                _ => "",
            },
        }
    }
}

pub fn run_tui() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Ok(());
            }

            match app.mode {
                Mode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('o') => {
                        app.scan_case_files();
                        if app.file_list.is_empty() {
                            app.status_msg = String::from("No .raw files found in cases/");
                        } else {
                            app.file_list_state = ListState::default();
                            app.file_list_state.select(Some(0));
                            app.mode = Mode::FileBrowser;
                            app.status_msg = String::from("Select a case file");
                        }
                    }
                    KeyCode::Tab => {
                        app.active_tab = app.active_tab.next();
                        app.table_state = TableState::default();
                        if app.row_count() > 0 {
                            app.table_state.select(Some(0));
                        }
                    }
                    KeyCode::BackTab => {
                        app.active_tab = app.active_tab.prev();
                        app.table_state = TableState::default();
                        if app.row_count() > 0 {
                            app.table_state.select(Some(0));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let count = app.row_count();
                        if count > 0 {
                            let i = app.table_state.selected().unwrap_or(0);
                            app.table_state.select(Some((i + 1).min(count - 1)));
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let count = app.row_count();
                        if count > 0 {
                            let i = app.table_state.selected().unwrap_or(0);
                            app.table_state.select(Some(i.saturating_sub(1)));
                        }
                    }
                    KeyCode::Char('r') => app.run_dc(),
                    KeyCode::Char('e') => app.start_edit(),
                    KeyCode::Char('s') => app.toggle_status(),
                    KeyCode::Char('b') => {
                        if app.active_tab == Tab::Buses
                            && app.network.is_some()
                            && app.table_state.selected().is_some()
                        {
                            app.bus_display_scroll = 0;
                            app.mode = Mode::BusDisplay;
                        }
                    }
                    _ => {}
                },
                Mode::FileBrowser => match key.code {
                    KeyCode::Esc => {
                        app.mode = Mode::Normal;
                        app.status_msg = String::new();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let len = app.file_list.len();
                        if len > 0 {
                            let i = app.file_list_state.selected().unwrap_or(0);
                            app.file_list_state.select(Some((i + 1).min(len - 1)));
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = app.file_list_state.selected().unwrap_or(0);
                        app.file_list_state.select(Some(i.saturating_sub(1)));
                    }
                    KeyCode::Enter => {
                        if let Some(i) = app.file_list_state.selected() {
                            let filename = app.file_list[i].clone();
                            app.mode = Mode::Normal;
                            app.load_case(&filename);
                        }
                    }
                    _ => {}
                },
                Mode::BusDisplay => match key.code {
                    KeyCode::Esc | KeyCode::Char('b') | KeyCode::Char('q') => {
                        app.mode = Mode::Normal;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.bus_display_scroll = app.bus_display_scroll.saturating_add(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.bus_display_scroll = app.bus_display_scroll.saturating_sub(1);
                    }
                    _ => {}
                },
                Mode::Editing => match key.code {
                    KeyCode::Esc => {
                        app.editing = None;
                        app.mode = Mode::Normal;
                        app.status_msg = String::from("Edit cancelled");
                    }
                    KeyCode::Enter => {
                        app.apply_edit();
                        app.editing = None;
                        app.mode = Mode::Normal;
                    }
                    KeyCode::Left => {
                        if let Some(ref edit) = app.editing
                            && edit.col > 0
                        {
                            let new_col = edit.col - 1;
                            let row = edit.row;
                            let val = app.cell_value(row, new_col);
                            let edit = app.editing.as_mut().unwrap();
                            edit.col = new_col;
                            edit.buf = val;
                        }
                    }
                    KeyCode::Right => {
                        if let Some(ref edit) = app.editing
                            && edit.col + 1 < edit.col_count
                        {
                            let new_col = edit.col + 1;
                            let row = edit.row;
                            let val = app.cell_value(row, new_col);
                            let edit = app.editing.as_mut().unwrap();
                            edit.col = new_col;
                            edit.buf = val;
                        }
                    }
                    KeyCode::Backspace => {
                        if let Some(ref mut edit) = app.editing {
                            edit.buf.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if let Some(ref mut edit) = app.editing {
                            edit.buf.push(c);
                        }
                    }
                    _ => {}
                },
            }
        }
    }
}

fn draw_ui(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    // Title bar
    let case_name = app
        .network
        .as_ref()
        .map(|n| n.case_name.as_str())
        .unwrap_or("No case loaded");
    let title = Block::default()
        .title(format!("  Mantis Power Systems  |  {}  ", case_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(title, chunks[0]);

    // Tabs
    let tab_titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|t| {
            let style = if *t == app.active_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(t.label(), style))
        })
        .collect();
    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(app.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[1]);

    // Table area
    draw_table(f, app, chunks[2]);

    // Status bar
    let keys = "  q:Quit  o:Open  r:Run DC  e:Edit  s:Toggle  b:Bus  Tab:Switch  j/k:Nav";
    let status_text = format!("  {}  |{}", app.status_msg, keys);
    let status =
        Paragraph::new(status_text).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    f.render_widget(status, chunks[3]);

    // Popups
    if app.mode == Mode::FileBrowser {
        draw_file_browser(f, app, area);
    }
    if app.mode == Mode::BusDisplay {
        draw_bus_display(f, app, area);
    }
}

fn draw_table(f: &mut Frame, app: &mut App, area: Rect) {
    let net = match &app.network {
        Some(n) => n,
        None => {
            let empty = Paragraph::new("  No case loaded. Press 'o' to open a file.")
                .block(Block::default().borders(Borders::ALL).title(" Data "));
            f.render_widget(empty, area);
            return;
        }
    };

    match app.active_tab {
        Tab::Buses => {
            let header = Row::new(vec![
                "ID", "Name", "Type", "Voltage", "Angle", "Nom kV", "P mis", "Q mis", "Status",
            ])
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);
            let rows: Vec<Row> = net
                .buses
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    let (p_mis, q_mis) = net.bus_mismatch(b.bus_id);
                    let cells = vec![
                        Cell::from(b.bus_id.to_string()),
                        Cell::from(b.bus_name.clone()),
                        Cell::from(format!("{}", b.bus_type)),
                        Cell::from(format!("{:.6}", b.voltage)),
                        Cell::from(format!("{:.4}", b.angle)),
                        Cell::from(format!("{:.1}", b.nom_voltage)),
                        Cell::from(format!("{:.2}", p_mis)),
                        Cell::from(format!("{:.2}", q_mis)),
                        Cell::from(if b.bus_status { "In" } else { "Out" }),
                    ];
                    style_row(Row::new(cells), app, i)
                })
                .collect();
            let table = Table::new(
                rows,
                [
                    Constraint::Length(6),
                    Constraint::Length(16),
                    Constraint::Length(5),
                    Constraint::Length(12),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(6),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Buses ({}) ", net.buses.len())),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray));
            f.render_stateful_widget(table, area, &mut app.table_state);
        }
        Tab::Branches => {
            let header = Row::new(vec![
                "ID", "From", "To", "Type", "R", "X", "RateA", "Flow", "Status",
            ])
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);
            let rows: Vec<Row> = net
                .branches
                .iter()
                .enumerate()
                .map(|(i, br)| {
                    let cells = vec![
                        Cell::from(br.id.to_string()),
                        Cell::from(br.from_bus.to_string()),
                        Cell::from(br.to_bus.to_string()),
                        Cell::from(format!("{}", br.branch_type)),
                        Cell::from(format!("{:.6}", br.resistance)),
                        Cell::from(format!("{:.6}", br.reactance)),
                        Cell::from(format!("{:.1}", br.operating_limit)),
                        Cell::from(format!("{:.3}", br.flow)),
                        Cell::from(if br.branch_status { "In" } else { "Out" }),
                    ];
                    style_row(Row::new(cells), app, i)
                })
                .collect();
            let table = Table::new(
                rows,
                [
                    Constraint::Length(6),
                    Constraint::Length(8),
                    Constraint::Length(8),
                    Constraint::Length(5),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Length(10),
                    Constraint::Length(12),
                    Constraint::Length(6),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Branches ({}) ", net.branches.len())),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray));
            f.render_stateful_widget(table, area, &mut app.table_state);
        }
        Tab::Generators => {
            let header = Row::new(vec![
                "ID", "Bus", "Name", "P", "Q", "Vset", "Pmin", "Pmax", "Status",
            ])
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);
            let rows: Vec<Row> = net
                .generators
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    let cells = vec![
                        Cell::from(g.gen_id.to_string()),
                        Cell::from(g.gen_bus_id.to_string()),
                        Cell::from(g.gen_name.clone()),
                        Cell::from(format!("{:.3}", g.p_gen)),
                        Cell::from(format!("{:.3}", g.q_gen)),
                        Cell::from(format!("{:.5}", g.v_setpoint)),
                        Cell::from(format!("{:.3}", g.p_min)),
                        Cell::from(format!("{:.3}", g.p_max)),
                        Cell::from(if g.gen_status { "In" } else { "Out" }),
                    ];
                    style_row(Row::new(cells), app, i)
                })
                .collect();
            let table = Table::new(
                rows,
                [
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(18),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(10),
                    Constraint::Length(6),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Generators ({}) ", net.generators.len())),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray));
            f.render_stateful_widget(table, area, &mut app.table_state);
        }
        Tab::Loads => {
            let header = Row::new(vec!["ID", "Bus", "Name", "P (MW)", "Q (MVAR)"])
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .bottom_margin(1);
            let rows: Vec<Row> = net
                .loads
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let cells = vec![
                        Cell::from(l.load_id.to_string()),
                        Cell::from(l.bus_id.to_string()),
                        Cell::from(l.load_name.clone()),
                        Cell::from(format!("{:.3}", l.real_load)),
                        Cell::from(format!("{:.3}", l.imag_load)),
                    ];
                    style_row(Row::new(cells), app, i)
                })
                .collect();
            let table = Table::new(
                rows,
                [
                    Constraint::Length(6),
                    Constraint::Length(8),
                    Constraint::Length(20),
                    Constraint::Length(12),
                    Constraint::Length(12),
                ],
            )
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Loads ({}) ", net.loads.len())),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray));
            f.render_stateful_widget(table, area, &mut app.table_state);
        }
    }

    // Edit overlay at bottom if editing
    if app.mode == Mode::Editing
        && let Some(ref edit) = app.editing
    {
        let edit_area = Rect::new(area.x, area.bottom().saturating_sub(3), area.width, 3);
        let label = app.col_label(edit.col);
        let text = format!(" {} > {} ", label, edit.buf);
        let p = Paragraph::new(text)
            .style(Style::default().fg(Color::White).bg(Color::Blue))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Edit Field (Left/Right to switch) ")
                    .border_style(Style::default().fg(Color::Yellow)),
            );
        f.render_widget(p, edit_area);
    }
}

fn style_row<'a>(row: Row<'a>, app: &App, index: usize) -> Row<'a> {
    if app.mode == Mode::Editing
        && let Some(ref edit) = app.editing
        && edit.row == index
    {
        return row.style(Style::default().fg(Color::Yellow));
    }
    row
}

fn draw_file_browser(f: &mut Frame, app: &mut App, area: Rect) {
    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = (app.file_list.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .file_list
        .iter()
        .map(|name| ListItem::new(format!("  {}", name)))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Open Case File (Enter to select, Esc to cancel) "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, popup_area, &mut app.file_list_state);
}

fn draw_bus_display(f: &mut Frame, app: &mut App, area: Rect) {
    let row = match app.table_state.selected() {
        Some(r) => r,
        None => return,
    };
    let net = match &app.network {
        Some(n) => n,
        None => return,
    };
    if row >= net.buses.len() {
        return;
    }
    let bus = &net.buses[row];

    let mut lines: Vec<Line> = Vec::new();

    // Bus header
    lines.push(Line::from(vec![
        Span::styled(
            "Bus ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            bus.bus_id.to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(&bus.bus_name, Style::default().fg(Color::White)),
    ]));
    lines.push(Line::raw(""));

    // Voltage and angle
    let status_color = if bus.bus_status {
        Color::Green
    } else {
        Color::Red
    };
    lines.push(Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if bus.bus_status {
                "In Service"
            } else {
                "Out of Service"
            },
            Style::default().fg(status_color),
        ),
        Span::raw("    "),
        Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", bus.bus_type),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Voltage: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.6} pu", bus.voltage),
            Style::default().fg(Color::White),
        ),
        Span::raw("    "),
        Span::styled("Angle: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.4} deg", bus.angle),
            Style::default().fg(Color::White),
        ),
        Span::raw("    "),
        Span::styled("Nom: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1} kV", bus.nom_voltage),
            Style::default().fg(Color::White),
        ),
    ]));

    // Generators on this bus
    let gens: Vec<_> = net
        .generators
        .iter()
        .filter(|g| g.gen_bus_id == bus.bus_id)
        .collect();
    if !gens.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("--- Generators ({}) ---", gens.len()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        for g in &gens {
            let st = if g.gen_status { "In" } else { "Out" };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", g.gen_name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(
                        "P={:.1} MW  Q={:.1} MVAR  Vset={:.4}  [{}]",
                        g.p_gen, g.q_gen, g.v_setpoint, st
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Loads on this bus
    let loads: Vec<_> = net
        .loads
        .iter()
        .filter(|l| l.bus_id == bus.bus_id)
        .collect();
    if !loads.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("--- Loads ({}) ---", loads.len()),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        for l in &loads {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", l.load_name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("P={:.1} MW  Q={:.1} MVAR", l.real_load, l.imag_load),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Net injection
    let p_gen: f32 = gens.iter().filter(|g| g.gen_status).map(|g| g.p_gen).sum();
    let p_load: f32 = loads.iter().map(|l| l.real_load).sum();
    let p_net = p_gen - p_load;
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("Net Injection: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1} MW", p_net),
            Style::default().fg(if p_net >= 0.0 {
                Color::Green
            } else {
                Color::Red
            }),
        ),
        Span::styled(
            format!("  (Gen={:.1}  Load={:.1})", p_gen, p_load),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    // Mismatch
    let (p_mis, q_mis) = net.bus_mismatch(bus.bus_id);
    let p_color = if p_mis.abs() < 0.1 {
        Color::Green
    } else {
        Color::Red
    };
    let q_color = if q_mis.abs() < 0.1 {
        Color::Green
    } else {
        Color::Red
    };
    lines.push(Line::from(vec![
        Span::styled("Mismatch:  ", Style::default().fg(Color::DarkGray)),
        Span::styled("P=", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.3} MW", p_mis), Style::default().fg(p_color)),
        Span::raw("    "),
        Span::styled("Q=", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.3} MVAR", q_mis), Style::default().fg(q_color)),
    ]));

    // Connected branches
    let branches: Vec<_> = net
        .branches
        .iter()
        .filter(|br| br.from_bus == bus.bus_id || br.to_bus == bus.bus_id)
        .collect();
    if !branches.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("--- Branches ({}) ---", branches.len()),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![Span::styled(
            "  To       Type  Flow (MW)    X          Rate     Status",
            Style::default().fg(Color::Yellow),
        )]));
        for br in &branches {
            let other = if br.from_bus == bus.bus_id {
                br.to_bus
            } else {
                br.from_bus
            };
            // Show flow direction: positive = into this bus convention depends on from/to
            let flow_display = if br.from_bus == bus.bus_id {
                br.flow
            } else {
                -br.flow
            };
            let st = if br.branch_status { "In" } else { "Out" };
            let flow_color = if !br.branch_status {
                Color::DarkGray
            } else if br.operating_limit > 0.0 && br.flow.abs() > br.operating_limit {
                Color::Red
            } else {
                Color::White
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<7}  {:<4}  ", other, br.branch_type),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{:>9.1}    ", flow_display),
                    Style::default().fg(flow_color),
                ),
                Span::styled(
                    format!(
                        "{:>9.6}  {:>7.1}    {}",
                        br.reactance, br.operating_limit, st
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Render as a centered popup
    let popup_width = 76u16.min(area.width.saturating_sub(4));
    let popup_height = (lines.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Bus Display (Esc to close) "),
        )
        .scroll((app.bus_display_scroll, 0));
    f.render_widget(para, popup_area);
}
