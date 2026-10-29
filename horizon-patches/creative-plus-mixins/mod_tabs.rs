// Creative tabs from content mods (creative-plus): every
// `data/<namespace>/creative_tab/<name>.json` of a content mod's namespace in
// the resource root becomes a tab `<namespace>:<name>`, and its items join
// the search tab. A content mod's files outside `data/<id>/horizon/` are
// copied into the root as they are, so no loader change is needed.
//
// {
//   "title": "itemGroup.ntm.parts",          translation key (optional)
//   "fallback": "NTM Resources and Parts",   shown when the key is unknown
//   "icon": "ntm:ingot_uranium",             the first item when left out
//   "order": 10,                             tabs sort by order, then id
//   "items": ["ntm:ingot_uranium", ...]      item ids, in the tab's order
// }
//
// Vanilla's tabs fill both rows, so mod tabs go on more pages, ten to a page
// (columns 1 to 5 of the top and bottom rows), as Forge pages them; the tabs
// of columns 6 and 7 (search, inventory, op blocks) stay on every page.
use super::{Creative, SearchEntry, Tab, java_lower, split_id};
use crate::items::ItemStack;
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Mod tabs on one page.
pub const PER_PAGE: usize = 10;
/// Where a tab of another page is "drawn": far off the screen, where no
/// click reaches it.
pub const HIDDEN: (f32, f32) = (-100_000.0, -100_000.0);

/// A mod tab: its id, page (1 and up) and English fallback title.
struct Placed {
    id: String,
    page: usize,
    fallback: String,
}

/// The root `Creative::load` read last, its mod tabs not added yet.
static LOADING: Mutex<Option<PathBuf>> = Mutex::new(None);
static PLACED: RwLock<Vec<Placed>> = RwLock::new(Vec::new());
static PAGE: AtomicUsize = AtomicUsize::new(0);
static PAGES: AtomicUsize = AtomicUsize::new(1);

/// `Creative::load(root)` starts.
pub fn loading(root: &Path) {
    *LOADING.lock().unwrap_or_else(|e| e.into_inner()) = Some(root.to_owned());
}

/// Before the creative screen draws or takes a click: the content mods'
/// tabs of the root loaded last, once.
pub fn ensure(creative: &mut Creative) {
    let root = LOADING.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(root) = root {
        add(creative, &root);
    }
}

/// Adds the content mods' tabs found in `root` to `creative`.
pub fn add(creative: &mut Creative, root: &Path) {
    let mut found: Vec<(i64, String, Value)> = Vec::new();
    for namespace in crate::mod_content::namespaces(root) {
        let dir = root.join("data").join(&namespace).join("creative_tab");
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let doc = fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
            match doc {
                Some(doc) => found.push((
                    doc["order"].as_i64().unwrap_or(0),
                    format!("{namespace}:{name}"),
                    doc,
                )),
                None => log::warn!("creative-plus: {} is not JSON", path.display()),
            }
        }
    }
    found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let mut placed = Vec::new();
    let mut searched: HashSet<String> = HashSet::new();
    for (_, id, doc) in found {
        let slot = placed.len() % PER_PAGE;
        let page = 1 + placed.len() / PER_PAGE;
        let title = doc["title"].as_str().unwrap_or(&id).to_owned();
        let fallback = doc["fallback"].as_str().unwrap_or(&title).to_owned();
        // Another `Game` (the server thread's) loading the same root into
        // this one's tabs: placed again, not added twice.
        if creative.tabs.iter().any(|t| t.id == id) {
            placed.push(Placed { id, page, fallback });
            continue;
        }
        let items: Vec<ItemStack> = doc["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|item| ItemStack::new(item, 1))
            .collect();
        if items.is_empty() {
            continue;
        }
        for stack in &items {
            if searched.insert(stack.item.to_string()) && creative.order_of(stack).is_none() {
                let (namespace, path) = split_id(&java_lower(&stack.item));
                creative.search_index.push(SearchEntry {
                    namespace,
                    path,
                    lines: Vec::new(),
                    tags: Vec::new(),
                });
                creative.search_items.push(stack.clone());
            }
        }
        let icon = doc["icon"]
            .as_str()
            .map_or_else(|| items[0].item.to_string(), str::to_owned);
        creative.tabs.push(Tab {
            id: id.clone(),
            title,
            icon,
            items,
            top: slot < PER_PAGE / 2,
            column: (slot % (PER_PAGE / 2)) as u32,
            search: false,
            inventory: false,
        });
        placed.push(Placed { id, page, fallback });
    }
    if !placed.is_empty() {
        log::info!("creative-plus: {} content mod tabs", placed.len());
    }
    PAGES.store(1 + placed.len().div_ceil(PER_PAGE), Ordering::Relaxed);
    PAGE.store(0, Ordering::Relaxed);
    *PLACED.write().unwrap_or_else(|e| e.into_inner()) = placed;
}

/// The page shown (0: vanilla's tabs) and how many there are.
pub fn page() -> usize {
    PAGE.load(Ordering::Relaxed)
}
pub fn pages() -> usize {
    PAGES.load(Ordering::Relaxed)
}

/// The page tab `id` is on: 0 for vanilla's.
fn page_of(id: &str) -> usize {
    let placed = PLACED.read().unwrap_or_else(|e| e.into_inner());
    placed.iter().find(|p| p.id == id).map_or(0, |p| p.page)
}

/// Whether `tab` shows on the page shown now.
pub fn on_page(tab: &Tab) -> bool {
    tab.column >= 5 || page_of(&tab.id) == page()
}

/// A mod tab's English title, for a title key the language lacks; None for
/// vanilla's tabs.
pub fn fallback(id: &str) -> Option<String> {
    let placed = PLACED.read().unwrap_or_else(|e| e.into_inner());
    placed.iter().find(|p| p.id == id).map(|p| p.fallback.clone())
}

/// Turns the page by `step` (wrapping around): the index of the tab to
/// select on it, its first.
pub fn turn(creative: &Creative, step: isize) -> Option<usize> {
    let pages = pages();
    if pages < 2 {
        return None;
    }
    let to = (page() as isize + step).rem_euclid(pages as isize) as usize;
    PAGE.store(to, Ordering::Relaxed);
    creative
        .tabs
        .iter()
        .position(|t| t.column < 5 && page_of(&t.id) == to)
}

/// The page buttons, left and right above the top tabs, relative to the
/// screen's origin: `[x, y, w, h]`.
pub fn buttons() -> [[f32; 4]; 2] {
    [
        [0.0, -50.0, 20.0, 20.0],
        [Creative::WIDTH - 20.0, -50.0, 20.0, 20.0],
    ]
}
