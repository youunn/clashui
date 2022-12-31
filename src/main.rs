use crossterm::{
	event,
	event::{Event, KeyCode},
	execute,
	terminal::{
		disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
		LeaveAlternateScreen,
	},
};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	error::Error,
	fmt, io,
	time::{Duration, Instant},
};
use tui::{
	backend::{Backend, CrosstermBackend},
	layout::{Constraint, Direction, Layout, Rect},
	style::{Color, Modifier, Style},
	text::{Span, Spans},
	widgets::{Block, Borders, List, ListItem, Tabs},
	Frame, Terminal,
};

#[derive(PartialEq, Debug)]
enum Route {
	General,
	Proxies,
	Rules,
	Connections,
	Logs,
}

impl fmt::Display for Route {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Debug::fmt(self, f)
	}
}

#[derive(PartialEq)]
enum Pane {
	Menu,
	Proxies,
	// Other,
}

const FRAGMENT: &AsciiSet =
	&CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

struct HttpClient {
	// TODO: async
	client: reqwest::blocking::Client,
	url: String,
}

impl HttpClient {
	fn new() -> Self {
		Self {
			client: Client::new(),
			url: String::from("http://localhost:9090"),
		}
	}

	// fn providers(&self) -> Result<HashMap<String, Proxy>, Box<dyn Error>> {
	// 	let res: ProviderList = self
	// 		.client
	// 		.get(format!("{}{}", self.url, "/providers/proxies"))
	// 		.send()?
	// 		.json()?;
	// 	Ok(res.providers)
	// }

	fn proxies(&self) -> Result<HashMap<String, Proxy>, Box<dyn Error>> {
		let res: ProxyList = self
			.client
			.get(format!("{}{}", self.url, "/proxies"))
			.send()?
			.json()?;
		Ok(res.proxies)
	}

	fn update_proxy(
		&self,
		provider: &str,
		name: &str,
	) -> Result<(), Box<dyn Error>> {
		let body = HashMap::from([("name", name)]);
		self.client
			.put(format!(
				"{}{}{}",
				self.url,
				"/proxies/",
				utf8_percent_encode(provider, FRAGMENT),
			))
			.json(&body)
			.send()?
			.json()?;
		Ok(())
	}
}

#[derive(Deserialize)]
struct ProxyList {
	proxies: HashMap<String, Proxy>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Proxy {
	all: Option<Vec<String>>,
	name: String,
	now: Option<String>,
}

impl Proxy {
	fn is_provider(&self) -> bool {
		self.all.is_some()
	}
}

#[derive(Default)]
struct ProxiesState {
	proxies: Option<HashMap<String, Proxy>>,
	provider: usize,
	proxy_index: usize,
	proxies_len: usize,
	providers_len: usize,
}

impl ProxiesState {
	fn fetch_data(&mut self, http: &HttpClient) {
		self.proxies = http.proxies().ok();
		if self.proxies.is_none() {
			self.provider = 0;
			self.proxy_index = 0;
			self.providers_len = 0;
			self.proxies_len = 0;
		} else {
			self.provider = 0;

			let providers = self.providers();
			let len = providers.len();

			if self.providers_len != 0 {
				self.proxies_len = providers[self.provider]
					.all
					.as_ref()
					.map(|v| v.len())
					.unwrap_or_default();
			} else {
				self.proxies_len = 0;
			}
			self.proxy_index = 0;

			self.providers_len = len;
		}
	}

	fn providers(&self) -> Vec<&Proxy> {
		let mut providers = if let Some(proxies) = &self.proxies {
			proxies.values().filter(|p| p.is_provider()).collect()
		} else {
			Vec::new()
		};

		providers.sort_by(|x, y| x.name.cmp(&y.name));

		providers
	}

	fn next_tab(&mut self) {
		if self.providers_len == 0 {
			self.provider = 0;
			return;
		}
		let index = self.provider + 1;
		self.provider = index % self.providers_len;
		let providers = self.providers();
		self.proxies_len = providers[self.provider]
			.all
			.as_ref()
			.map(|v| v.len())
			.unwrap_or_default();
		self.proxy_index = 0;
	}

	fn previous_tab(&mut self) {
		if self.providers_len == 0 {
			self.provider = 0;
			return;
		}
		let index = self.provider + self.providers_len - 1;
		self.provider = index % self.providers_len;
		let providers = self.providers();
		self.proxies_len = providers[self.provider]
			.all
			.as_ref()
			.map(|v| v.len())
			.unwrap_or_default();
		self.proxy_index = 0;
	}

	fn next_proxy(&mut self) {
		if self.proxies_len == 0 {
			self.proxy_index = 0;
			return;
		}
		let index = self.proxy_index + 1;
		self.proxy_index = index % self.proxies_len;
	}

	fn previous_proxy(&mut self) {
		if self.proxies_len == 0 {
			self.proxy_index = 0;
			return;
		}
		let index = self.proxy_index + self.proxies_len - 1;
		self.proxy_index = index % self.proxies_len;
	}

	fn select_proxy(&mut self, http: &HttpClient) {
		if self.providers_len == 0 || self.proxies_len == 0 {
			return;
		}

		let providers = self.providers();
		let provider_index = self.provider;
		let provider = match providers.get(provider_index) {
			Some(provider) => provider,
			_ => return,
		};
		let proxy_index = self.proxy_index;
		let name = match &provider.all {
			Some(proxies) => {
				let mut proxies: Vec<_> = proxies
					.iter()
					.map(|s| &**s)
					.collect();
				proxies.sort();

				match proxies.get(proxy_index) {
					Some(proxy) => *proxy,
					_ => return,
				}
			}
			_ => return,
		};

		http.update_proxy(&provider.name, name).ok();
		self.fetch_data(http);

		if self.providers_len == 0 || self.proxies_len == 0 {
			return;
		}
		if provider_index < self.providers_len {
			self.provider = provider_index;
		}
		if proxy_index < self.proxies_len {
			self.proxy_index = proxy_index;
		}
	}
}

struct App {
	http: HttpClient,
	routes: Vec<Route>,
	page: usize,
	focus: Pane,
	proxies_state: ProxiesState,
}

impl Default for App {
	fn default() -> Self {
		let routes = vec![
			Route::General,
			Route::Proxies,
			Route::Rules,
			Route::Connections,
			Route::Logs,
		];

		Self {
			http: HttpClient::new(),
			routes,
			page: 0,
			focus: Pane::Menu,
			proxies_state: ProxiesState::default(),
		}
	}
}

impl App {
	fn navigate(&mut self, page: usize) {
		self.page = page % self.routes.len();
		self.fetch_data();
	}

	fn next_menu(&mut self) {
		let page = self.page + 1;
		self.page = page % self.routes.len();
		self.fetch_data();
	}

	fn previous_menu(&mut self) {
		let page = self.page + self.routes.len() - 1;
		self.page = page % self.routes.len();
		self.fetch_data();
	}

	fn fetch_data(&mut self) {
		let route = match self.route() {
			Some(route) => route,
			_ => return,
		};
		match route {
			Route::General => {}
			Route::Proxies => {
				self.proxies_state.fetch_data(&self.http)
			}
			Route::Rules => {}
			Route::Connections => {}
			Route::Logs => {}
		}
	}

	fn route(&self) -> Option<&Route> {
		self.routes.get(self.page)
	}
}

fn main() -> Result<(), Box<dyn Error>> {
	// TODO: log
	// TODO: process arguments

	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;

	let tick_rate = Duration::from_secs(1);
	let app = App::default();
	let res = run_app(&mut terminal, app, tick_rate);

	disable_raw_mode()?;
	execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
	terminal.show_cursor()?;

	if let Err(err) = res {
		println!("{:?}", err)
	}

	Ok(())
}

fn run_app<B: Backend>(
	terminal: &mut Terminal<B>,
	mut app: App,
	tick_rate: Duration,
) -> io::Result<()> {
	let mut last_tick = Instant::now();
	loop {
		terminal.draw(|f| render(f, &mut app))?;

		let timeout = tick_rate
			.checked_sub(last_tick.elapsed())
			.unwrap_or_else(|| Duration::from_secs(0));

		if event::poll(timeout)? {
			if let Event::Key(key) = event::read()? {
				let res = process_key(key.code, &mut app);
				match res {
					ProcessResult::Noop => {}
					ProcessResult::Ok => return Ok(()),
				}
			}
		}

		if last_tick.elapsed() >= tick_rate {
			last_tick = Instant::now();
		}
	}
}

enum ProcessResult {
	Noop,
	Ok,
	// Error,
}

fn process_key(code: KeyCode, app: &mut App) -> ProcessResult {
	if let KeyCode::Char('q') = code {
		return ProcessResult::Ok;
	}

	let focus = &app.focus;
	match focus {
		Pane::Menu => match code {
			KeyCode::Char('j') => app.next_menu(),
			KeyCode::Char('k') => app.previous_menu(),
			KeyCode::Char('l') => {
				if app.route() == Some(&Route::Proxies) {
					app.focus = Pane::Proxies;
					app.fetch_data();
				}
			}
			KeyCode::Char('1') => app.navigate(0),
			KeyCode::Char('2') => app.navigate(1),
			KeyCode::Char('3') => app.navigate(2),
			KeyCode::Char('4') => app.navigate(3),
			KeyCode::Char('5') => app.navigate(4),
			_ => {}
		},
		Pane::Proxies => match code {
			KeyCode::Esc | KeyCode::Char('h') => {
				app.focus = Pane::Menu;
			}
			KeyCode::Char(' ') => {
				app.proxies_state.select_proxy(&app.http);
			}
			KeyCode::Char('j') => {
				app.proxies_state.next_proxy();
			}
			KeyCode::Char('k') => {
				app.proxies_state.previous_proxy();
			}
			KeyCode::Char('H') => {
				app.proxies_state.previous_tab();
			}
			KeyCode::Char('L') => {
				app.proxies_state.next_tab();
			}
			_ => {}
		},
		// _ => match code {
		// 	KeyCode::Esc | KeyCode::Char('h') => {
		// 		app.focus = Pane::Menu;
		// 	}
		// 	_ => {}
		// },
	}

	ProcessResult::Noop
}

fn render<B: Backend>(f: &mut Frame<B>, app: &mut App) {
	let chunks = Layout::default()
		.direction(Direction::Horizontal)
		.constraints(
			[
				Constraint::Percentage(30),
				Constraint::Percentage(70),
			]
			.as_ref(),
		)
		.split(f.size());

	let items = &app.routes;
	let page = app.page;
	let menu = draw_menu(items, page);
	f.render_widget(menu, chunks[0]);

	let route = &app.routes.get(app.page).unwrap_or(&Route::General);
	let proxies_state = &mut app.proxies_state;
	let focus = &app.focus;
	render_main(f, route, proxies_state, focus, chunks[1]);
}

fn draw_menu(items: &[Route], page: usize) -> List<'_> {
	let items: Vec<_> = items
		.iter()
		.map(|route| {
			let name = route.to_string();

			let style = if items.get(page) == Some(route) {
				Style::default()
					.bg(Color::LightBlue)
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default()
			};

			let spans = Spans::from(Span::styled(
				name,
				Style::default().add_modifier(Modifier::ITALIC),
			));

			ListItem::new(spans).style(style)
		})
		.collect();

	let menu = List::new(items)
		.block(Block::default().borders(Borders::ALL).title("Clash"));

	menu
}

fn render_main<'a, B: Backend>(
	f: &'a mut Frame<B>,
	route: &'a Route,
	state: &mut ProxiesState,
	focus: &'a Pane,
	rect: Rect,
) {
	match route {
		Route::General => f.render_widget(draw_general(), rect),
		Route::Proxies => render_proxies(f, state, focus, rect),
		Route::Rules => f.render_widget(draw_rules(), rect),
		Route::Connections => f.render_widget(draw_connections(), rect),
		Route::Logs => f.render_widget(draw_logs(), rect),
	}
}

fn draw_general<'a>() -> Block<'a> {
	Block::default().borders(Borders::ALL).title("General")
}

fn render_proxies<'a, B: Backend>(
	f: &'a mut Frame<B>,
	state: &mut ProxiesState,
	focus: &'a Pane,
	rect: Rect,
) {
	let chunks = Layout::default()
		.direction(Direction::Vertical)
		.constraints(
			[Constraint::Length(3), Constraint::Min(0)].as_ref(),
		)
		.split(rect);

	let block = Block::default().style(Style::default());
	f.render_widget(block, rect);

	if state.providers_len == 0 {
		return;
	}

	let providers = state.providers();

	let titles: Vec<_> = providers
		.iter()
		.skip(state.provider)
		.map(|p| Spans::from(p.name.as_ref()))
		.collect();

	let mut tabs = Tabs::new(titles)
		.block(Block::default().borders(Borders::ALL).title("Proxies"))
		.style(Style::default())
		.highlight_style(Style::default().add_modifier(Modifier::BOLD));

	tabs = tabs.select(0);
	if focus == &Pane::Proxies {
		tabs = tabs.highlight_style(
			Style::default()
				.fg(Color::LightBlue)
				.add_modifier(Modifier::BOLD),
		);
	}

	f.render_widget(tabs, chunks[0]);

	let provider = providers[state.provider];
	let mut titles: Vec<_> = provider
		.all
		.as_ref()
		.map(|v| v.iter().map(|s| &**s).collect())
		.unwrap_or_default();

	titles.sort();
	let items: Vec<_> = titles
		.iter()
		.skip(state.proxy_index)
		.enumerate()
		.map(|(i, &t)| {
			let mut style = Style::default();
			if Some(t) == provider.now.as_deref() {
				style = style
					.fg(Color::LightRed)
					.add_modifier(Modifier::BOLD);
			}
			if i == 0 {
				style = style.bg(Color::LightBlue);
			}
			ListItem::new(Spans::from(t)).style(style)
		})
		.collect();

	let block = Block::default()
		.borders(Borders::ALL)
		.style(Style::default());
	let list = List::new(items).block(block);

	f.render_widget(list, chunks[1]);
}

fn draw_rules<'a>() -> Block<'a> {
	Block::default().borders(Borders::ALL).title("Rules")
}

fn draw_connections<'a>() -> Block<'a> {
	Block::default().borders(Borders::ALL).title("Connections")
}

fn draw_logs<'a>() -> Block<'a> {
	Block::default().borders(Borders::ALL).title("Logs")
}
