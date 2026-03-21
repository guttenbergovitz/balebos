use ratatui::widgets::TableState;
use crate::storage::JsonlStorage;
use crate::tui::model::{load_tui_model, TuiModel, AssetState};
use std::collections::HashSet;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

pub enum VisibleItem {
    Root(String),
    Subdomain(String, String),
}

pub struct App {
    pub model: TuiModel,
    pub table_state: TableState,
    pub expanded_roots: HashSet<String>,
    pub visible_items: Vec<VisibleItem>,
    pub should_quit: bool,
    pub storage: JsonlStorage,
    
    // Interactive Scan
    pub input: String,
    pub input_mode: InputMode,
    pub is_scanning: Arc<Mutex<bool>>,
}

impl App {
    pub fn new(storage: JsonlStorage) -> Result<Self, io::Error> {
        let model = load_tui_model(&storage)?;
        let mut app = Self {
            model,
            table_state: TableState::default(),
            expanded_roots: HashSet::new(),
            visible_items: Vec::new(),
            should_quit: false,
            storage,
            input: String::new(),
            input_mode: InputMode::Normal,
            is_scanning: Arc::new(Mutex::new(false)),
        };
        app.update_visible_items();
        if !app.visible_items.is_empty() {
            app.table_state.select(Some(0));
        }
        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<(), io::Error> {
        self.model = load_tui_model(&self.storage)?;
        self.update_visible_items();
        self.fix_selection();
        Ok(())
    }

    pub fn update_visible_items(&mut self) {
        let mut items = Vec::new();
        for group in &self.model.groups {
            items.push(VisibleItem::Root(group.root.name.clone()));
            if self.expanded_roots.contains(&group.root.name) {
                for sub in &group.subdomains {
                    items.push(VisibleItem::Subdomain(group.root.name.clone(), sub.name.clone()));
                }
            }
        }
        self.visible_items = items;
    }

    pub fn toggle_expand(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if let Some(item) = self.visible_items.get(selected) {
                match item {
                    VisibleItem::Root(name) => {
                        if self.expanded_roots.contains(name) {
                            self.expanded_roots.remove(name);
                        } else {
                            self.expanded_roots.insert(name.clone());
                        }
                        self.update_visible_items();
                        self.fix_selection();
                    }
                    VisibleItem::Subdomain(_, _) => {}
                }
            }
        }
    }

    fn fix_selection(&mut self) {
        if let Some(selected) = self.table_state.selected() {
            if selected >= self.visible_items.len() {
                if self.visible_items.is_empty() {
                    self.table_state.select(None);
                } else {
                    self.table_state.select(Some(self.visible_items.len() - 1));
                }
            }
        } else if !self.visible_items.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    pub fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.visible_items.len() - 1 { 0 } else { i + 1 }
            }
            None => 0,
        };
        if !self.visible_items.is_empty() {
            self.table_state.select(Some(i));
        }
    }

    pub fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 { self.visible_items.len() - 1 } else { i - 1 }
            }
            None => 0,
        };
        if !self.visible_items.is_empty() {
            self.table_state.select(Some(i));
        }
    }

    pub fn selected_asset(&self) -> Option<&AssetState> {
        let index = self.table_state.selected()?;
        let item = self.visible_items.get(index)?;
        match item {
            VisibleItem::Root(name) => self.model.groups.iter().find(|g| g.root.name == *name).map(|g| &g.root),
            VisibleItem::Subdomain(root_name, sub_name) => {
                self.model.groups.iter()
                    .find(|g| g.root.name == *root_name)?
                    .subdomains.iter()
                    .find(|s| s.name == *sub_name)
            }
        }
    }

    pub fn start_scan(&mut self) {
        let domain = self.input.drain(..).collect::<String>();
        if domain.is_empty() { return; }
        
        self.input_mode = InputMode::Normal;
        self.run_background_recon(domain);
    }

    pub fn start_rescan(&mut self) {
        if let Some(asset) = self.selected_asset() {
            let name = asset.name.clone();
            self.run_background_recon(name);
        }
    }

    fn run_background_recon(&self, target: String) {
        let storage = self.storage.clone();
        let is_scanning = Arc::clone(&self.is_scanning);
        
        {
            let mut scanning = is_scanning.lock().unwrap();
            *scanning = true;
        }

        thread::spawn(move || {
            // Check if it's a root domain or subdomain
            // For root domains we might want full discovery, for subdomains just probing.
            // For simplicity, we run a focused scan on this target.
            
            let _ = crate::app::probe_dns::probe_dns(&target, &storage);
            let _ = crate::app::probe_http::http_check(&target, &storage);

            let mut scanning = is_scanning.lock().unwrap();
            *scanning = false;
        });
    }
}
