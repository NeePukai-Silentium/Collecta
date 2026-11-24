//! Reactions of the mockup: navigation, filters, selection, menus, dialogs, quick view, a pretend scan.
//! It only changes the made-up data in memory (mock.rs), never files.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};

use crate::gallery::{self, Params};
use crate::mock::{self, Data, Photo, date_text, plural, size_text};
use crate::thumbs::{self, Pixels};
use crate::{
    Album, AlbumGroup, AppState, CategoryItem, DupFile, DupGroup, Gallery, GalleryKind, Icons, ImportLibrary,
    LibraryCard, MainWindow, MenuEntry, Page, QuickInfo, TagItem, Theme, Thumb, Tile,
};

mod browse;
mod dialogs;
mod menus;
mod scan;

/// A click waits this long for a second one: double click opens Pictora, a single one — quick view.
const DOUBLE_CLICK: Duration = Duration::from_millis(250);
const TOAST_TIME: Duration = Duration::from_secs(2);
const SCAN_STEP: Duration = Duration::from_millis(50);
const RECENT_COUNT: usize = 16;
const GALLERIES: [GalleryKind; 3] = [GalleryKind::All, GalleryKind::Favorites, GalleryKind::Album];

fn index_of(kind: GalleryKind) -> usize {
    match kind {
        GalleryKind::Favorites => 1,
        GalleryKind::Album => 2,
        _ => 0,
    }
}

#[derive(Default)]
struct View {
    ids: Vec<usize>,
    params: Option<Params>,
    tiles: Rc<VecModel<Tile>>,
}

struct Quick {
    ids: Vec<usize>,
    position: usize,
    /// Tells apart the full picture of the current photo from late ones of photos shown before.
    generation: u64,
}

struct PendingClick {
    kind: GalleryKind,
    id: usize,
    at: Instant,
    rect: [f32; 4],
}

/// What the open context menu or dropdown does with a click on row `index`.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    QuickView,
    Pictora,
    AddToAlbum,
    Tags,
    Category,
    Favorite,
    Rename,
    Move,
    Explorer,
    Delete,
    Sort(i32),
    AllCategories,
    FilterCategory(usize),
    AllTags,
    FilterTag(usize),
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum Submenu {
    Albums,
    Tags,
    Category,
}

#[derive(Default)]
struct Scan {
    step: u32,
}

pub struct App {
    window: MainWindow,
    me: Weak<App>,
    data: RefCell<Data>,
    picture_paths: Vec<PathBuf>,
    pictures: RefCell<Vec<Option<Image>>>,
    pictures_loaded: Cell<usize>,
    removed: RefCell<HashSet<usize>>,
    views: RefCell<[View; 3]>,
    recent: RefCell<Vec<usize>>,
    selection: RefCell<BTreeSet<usize>>,
    anchor: Cell<Option<usize>>,
    categories: RefCell<Vec<usize>>,
    tags: RefCell<Vec<usize>>,
    album: Cell<usize>,
    quick: RefCell<Option<Quick>>,
    quick_generation: Cell<u64>,
    pending: RefCell<Option<PendingClick>>,
    actions: RefCell<Vec<Action>>,
    submenu: Cell<Option<Submenu>>,
    exact_shown: RefCell<Vec<usize>>,
    similar_shown: RefCell<Vec<usize>>,
    dialog_target: Cell<(bool, i32)>,
    new_library: RefCell<(Vec<String>, Vec<String>)>,
    folder_counter: Cell<u32>,
    scan: RefCell<Scan>,
    started: Instant,
    click_timer: Timer,
    toast_timer: Timer,
    refresh_timer: Timer,
    scan_timer: Timer,
}

impl App {
    pub fn new(window: &MainWindow, picture_paths: Vec<PathBuf>) -> Rc<App> {
        let data = mock::generate(picture_paths.len());
        let pictures = vec![None; picture_paths.len()];
        let app = Rc::new_cyclic(|me| App {
            window: window.clone_strong(),
            me: me.clone(),
            data: RefCell::new(data),
            picture_paths,
            pictures: RefCell::new(pictures),
            pictures_loaded: Cell::new(0),
            removed: RefCell::default(),
            views: RefCell::default(),
            recent: RefCell::default(),
            selection: RefCell::default(),
            anchor: Cell::new(None),
            categories: RefCell::default(),
            tags: RefCell::default(),
            album: Cell::new(0),
            quick: RefCell::new(None),
            quick_generation: Cell::new(0),
            pending: RefCell::new(None),
            actions: RefCell::default(),
            submenu: Cell::new(None),
            exact_shown: RefCell::default(),
            similar_shown: RefCell::default(),
            dialog_target: Cell::new((false, -1)),
            new_library: RefCell::default(),
            folder_counter: Cell::new(1),
            scan: RefCell::default(),
            started: Instant::now(),
            click_timer: Timer::default(),
            toast_timer: Timer::default(),
            refresh_timer: Timer::default(),
            scan_timer: Timer::default(),
        });
        app.connect(window);
        app.refresh_all();
        app
    }

    fn ui(&self) -> &MainWindow {
        &self.window
    }

    fn state(&self) -> AppState<'_> {
        self.window.global::<AppState>()
    }

    /// Runs `action` on the app later, from a timer or a background thread.
    fn later(&self) -> Weak<App> {
        self.me.clone()
    }

    fn connect(&self, window: &MainWindow) {
        let state = window.global::<AppState>();
        macro_rules! on {
            ($setter:ident, |$app:ident $(, $arg:ident)*| $body:expr) => {{
                let me = self.later();
                state.$setter(move |$($arg),*| {
                    if let Some($app) = me.upgrade() {
                        $body
                    }
                });
            }};
        }
        on!(on_navigate, |app, page| app.navigate(page));
        on!(on_select_library, |app, index| app.select_library(index));
        on!(on_search_edited, |app, _text| {
            if !matches!(app.state().get_page(), Page::All | Page::Favorites | Page::Album) {
                app.navigate(Page::All);
            }
            app.refresh_views();
        });
        on!(on_view_changed, |app| app.refresh_views());
        on!(on_relayout, |app, kind, width, thumb, caption, gap, masonry| {
            app.views.borrow_mut()[index_of(kind)].params = Some(Params { width, thumb, caption, gap, masonry });
            app.refresh_view(kind);
        });
        on!(on_tile_clicked, |app, kind, id, control, shift, x, y, w, h| {
            app.tile_clicked(kind, id as usize, control, shift, [x, y, w, h])
        });
        on!(on_tile_context, |app, kind, id, x, y| app.tile_context(kind, id as usize, x, y));
        on!(on_quick_open_selected, |app, kind| app.quick_open_selected(kind));
        on!(on_quick_step, |app, delta| app.quick_step(delta));
        on!(on_quick_close, |app| app.quick_close());
        on!(on_quick_favorite, |app| app.quick_favorite());
        on!(on_quick_pictora, |app| app.toast("Открыть в Pictora — в макете не работает"));
        on!(on_quick_description_edited, |app, text| app.quick_description(text));
        on!(on_show_menu, |app, kind, x, y, width| app.show_menu(&kind, x, y, width));
        on!(on_menu_activated, |app, index| app.menu_activated(index as usize));
        on!(on_submenu_requested, |app, index| app.open_submenu(index as usize));
        on!(on_submenu_activated, |app, index| app.submenu_activated(index as usize));
        on!(on_remove_filter, |app, index| app.remove_filter(index as usize));
        on!(on_clear_filters, |app| {
            app.categories.borrow_mut().clear();
            app.tags.borrow_mut().clear();
            app.refresh_views();
        });
        on!(on_open_album, |app, id| app.open_album(id as usize));
        on!(on_create, |app, kind| app.ask_name(&kind));
        on!(on_tag_clicked, |app, name| app.tag_clicked(&name));
        on!(on_category_clicked, |app, index| {
            *app.categories.borrow_mut() = vec![index as usize];
            app.tags.borrow_mut().clear();
            app.navigate(Page::All);
        });
        on!(on_dup_keep, |app, similar, group, file| app.dup_keep(similar, group as usize, file as usize));
        on!(on_dup_remove, |app, similar, group| app.ask_dup_remove(similar, group));
        on!(on_scan_start, |app| app.scan_start());
        on!(on_scan_cancel, |app| app.scan_stop(true));
        on!(on_folder_add, |app, library, excluded| app.folder_add(library as usize, excluded));
        on!(on_folder_remove, |app, library, excluded, index| app.folder_remove(
            library as usize,
            excluded,
            index as usize
        ));
        on!(on_library_remove, |app, index| app.ask_library_remove(index as usize));
        on!(on_add_library, |app| app.add_library());
        on!(on_new_library_folder_add, |app, excluded| app.new_library_folder_add(excluded));
        on!(on_new_library_folder_remove, |app, excluded, index| {
            let mut lists = app.new_library.borrow_mut();
            let list = if excluded { &mut lists.1 } else { &mut lists.0 };
            if (index as usize) < list.len() {
                list.remove(index as usize);
            }
            drop(lists);
            app.show_new_library_folders();
        });
        on!(on_dialog_confirmed, |app| app.dialog_confirmed());
        on!(on_dialog_cancelled, |app| app.ui().global::<AppState>().set_dialog("".into()));
    }

    // ---- Test pictures ----

    pub fn picture_paths(&self) -> &[PathBuf] {
        &self.picture_paths
    }

    pub fn picture_loaded(&self, index: usize, width: u32, height: u32, thumbnail: Pixels) {
        self.pictures.borrow_mut()[index] = Some(Image::from_rgba8(thumbnail));
        for photo in self.data.borrow_mut().photos.iter_mut().filter(|photo| photo.picture == Some(index)) {
            photo.width = width;
            photo.height = height;
        }
        self.pictures_loaded.set(self.pictures_loaded.get() + 1);
        if self.pictures_loaded.get() == self.picture_paths.len() {
            crate::timing::write_log(&format!(
                "thumbnails: {} pictures in {} ms, {}",
                self.picture_paths.len(),
                self.started.elapsed().as_millis(),
                crate::timing::memory_usage()
            ));
        }
        // Pictures arrive one by one; showing them in batches keeps the feed from re-laying out on every one.
        if !self.refresh_timer.running() {
            let me = self.later();
            self.refresh_timer.start(TimerMode::SingleShot, Duration::from_millis(100), move || {
                if let Some(app) = me.upgrade() {
                    app.refresh_all();
                }
            });
        }
    }

    fn picture(&self, photo: &Photo) -> Option<Image> {
        photo.picture.and_then(|index| self.pictures.borrow()[index].clone())
    }

    // ---- Everything on screen ----

    fn refresh_all(&self) {
        self.refresh_views();
        self.refresh_library_cards();
        self.refresh_recent();
        self.refresh_albums();
        self.refresh_tags_and_categories();
        self.refresh_duplicates();
        self.refresh_import();
    }

    fn refresh_views(&self) {
        self.refresh_filter_labels();
        for kind in GALLERIES {
            self.refresh_view(kind);
        }
    }

    fn visible(&self, id: usize, kind: GalleryKind) -> bool {
        let data = self.data.borrow();
        let photo = &data.photos[id];
        let state = self.state();
        if self.removed.borrow().contains(&id) {
            return false;
        }
        match kind {
            GalleryKind::Favorites if !photo.favorite => return false,
            GalleryKind::Album if !photo.albums.contains(&self.album.get()) => return false,
            _ => {}
        }
        let library = state.get_library();
        if kind == GalleryKind::All && library >= 0 && photo.library != library as usize {
            return false;
        }
        let categories = self.categories.borrow();
        if !categories.is_empty() && !categories.contains(&photo.category) {
            return false;
        }
        let tags = self.tags.borrow();
        if !tags.is_empty() {
            let has = |tag: &usize| photo.tags.contains(tag);
            let fits = if state.get_tags_and() { tags.iter().all(has) } else { tags.iter().any(has) };
            if !fits {
                return false;
            }
        }
        let search = state.get_search().to_lowercase();
        let search = search.trim();
        if !search.is_empty() {
            let in_name = photo.name.to_lowercase().contains(search);
            let in_path = photo.folder.to_lowercase().contains(search);
            let in_tags = photo.tags.iter().any(|tag| data.tags[*tag].to_lowercase().contains(search));
            if !(in_name || in_path || in_tags) {
                return false;
            }
        }
        true
    }

    fn sorted_ids(&self, kind: GalleryKind) -> Vec<usize> {
        let count = self.data.borrow().photos.len();
        let mut ids: Vec<usize> = (0..count).filter(|id| self.visible(*id, kind)).collect();
        let data = self.data.borrow();
        let photos = &data.photos;
        match self.state().get_sort() {
            0 => ids.sort_by_key(|id| photos[*id].name.to_lowercase()),
            2 => ids.sort_by_key(|id| std::cmp::Reverse(photos[*id].bytes)),
            3 => ids.sort_by_key(|id| (photos[*id].format, photos[*id].name.to_lowercase())),
            _ => ids.sort_by_key(|id| std::cmp::Reverse(photos[*id].day)),
        }
        ids
    }

    fn refresh_view(&self, kind: GalleryKind) {
        let ids = self.sorted_ids(kind);
        let data = self.data.borrow();
        let selection = self.selection.borrow();
        let mut views = self.views.borrow_mut();
        let view = &mut views[index_of(kind)];
        view.ids = ids;
        let mut tiles = Vec::new();
        let mut height = 0.0;
        if let Some(params) = view.params {
            let ratios: Vec<f32> =
                view.ids.iter().map(|id| data.photos[*id].width as f32 / data.photos[*id].height as f32).collect();
            let (placed, total) = gallery::layout(&ratios, &params);
            height = total;
            for (id, place) in view.ids.iter().zip(placed) {
                let photo = &data.photos[*id];
                let image = self.picture(photo);
                tiles.push(Tile {
                    id: *id as i32,
                    has_image: image.is_some(),
                    image: image.unwrap_or_default(),
                    x: place.x,
                    y: place.y,
                    width: place.width,
                    height: place.height,
                    name: photo.name.clone().into(),
                    info: format!("{} × {} · {}", photo.width, photo.height, size_text(photo.bytes)).into(),
                    selected: selection.contains(id),
                    favorite: photo.favorite,
                });
            }
        }
        view.tiles.set_vec(tiles);
        let mut status = plural(view.ids.len(), "изображение", "изображения", "изображений");
        let selected = view.ids.iter().filter(|id| selection.contains(id)).count();
        if selected > 0 {
            status += &format!(" · выделено {selected}");
        }
        let gallery = Gallery { tiles: ModelRc::from(view.tiles.clone()), height, status: status.into() };
        let state = self.state();
        match kind {
            GalleryKind::Favorites => state.set_favorites_gallery(gallery),
            GalleryKind::Album => state.set_album_gallery(gallery),
            _ => state.set_all_gallery(gallery),
        }
    }

    /// Only the selection marks and the counter change: no re-layout.
    fn refresh_selection(&self) {
        let selection = self.selection.borrow();
        let state = self.state();
        for kind in GALLERIES {
            let views = self.views.borrow();
            let view = &views[index_of(kind)];
            for row in 0..view.tiles.row_count() {
                let Some(mut tile) = view.tiles.row_data(row) else {
                    continue;
                };
                let selected = selection.contains(&(tile.id as usize));
                if tile.selected != selected {
                    tile.selected = selected;
                    view.tiles.set_row_data(row, tile);
                }
            }
            let mut status = plural(view.ids.len(), "изображение", "изображения", "изображений");
            let selected = view.ids.iter().filter(|id| selection.contains(id)).count();
            if selected > 0 {
                status += &format!(" · выделено {selected}");
            }
            let gallery = match kind {
                GalleryKind::Favorites => state.get_favorites_gallery(),
                GalleryKind::Album => state.get_album_gallery(),
                _ => state.get_all_gallery(),
            };
            let gallery = Gallery { status: status.into(), ..gallery };
            match kind {
                GalleryKind::Favorites => state.set_favorites_gallery(gallery),
                GalleryKind::Album => state.set_album_gallery(gallery),
                _ => state.set_all_gallery(gallery),
            }
        }
    }

    fn refresh_filter_labels(&self) {
        let data = self.data.borrow();
        let state = self.state();
        let label = |chosen: &[usize], names: &[String]| -> SharedString {
            match chosen {
                [] => "Все".into(),
                [one] => names[*one].clone().into(),
                many => format!("выбрано {}", many.len()).into(),
            }
        };
        state.set_category_filter(label(&self.categories.borrow(), &data.categories));
        state.set_tag_filter(label(&self.tags.borrow(), &data.tags));
        let chips: Vec<SharedString> = self
            .categories
            .borrow()
            .iter()
            .map(|index| data.categories[*index].clone().into())
            .chain(self.tags.borrow().iter().map(|index| format!("#{}", data.tags[*index]).into()))
            .collect();
        state.set_filters(ModelRc::new(VecModel::from(chips)));
    }

    fn live_photos(&self) -> Vec<usize> {
        let removed = self.removed.borrow();
        (0..self.data.borrow().photos.len()).filter(|id| !removed.contains(id)).collect()
    }

    fn cover_of(&self, ids: impl Iterator<Item = usize>) -> (Image, bool) {
        let data = self.data.borrow();
        let mut ids = ids;
        match ids.find_map(|id| self.picture(&data.photos[id])) {
            Some(image) => (image, true),
            None => (Image::default(), false),
        }
    }

    fn refresh_library_cards(&self) {
        let live = self.live_photos();
        let data = self.data.borrow();
        let cards: Vec<LibraryCard> = data
            .libraries
            .iter()
            .enumerate()
            .map(|(index, library)| {
                let ids: Vec<usize> = live.iter().copied().filter(|id| data.photos[*id].library == index).collect();
                let bytes: u64 = ids.iter().map(|id| data.photos[*id].bytes).sum();
                let (cover, has_cover) = self.cover_of(ids.iter().copied());
                LibraryCard {
                    name: library.name.clone().into(),
                    total: ids.len().to_string().into(),
                    count: plural(ids.len(), "изображение", "изображения", "изображений").into(),
                    size: size_text(bytes).into(),
                    sources: plural(library.sources.len(), "источник", "источника", "источников").into(),
                    scanned: library.scanned.into(),
                    cover,
                    has_cover,
                }
            })
            .collect();
        self.state().set_libraries(ModelRc::new(VecModel::from(cards)));
    }

    fn refresh_recent(&self) {
        let mut ids = self.live_photos();
        let data = self.data.borrow();
        ids.sort_by_key(|id| std::cmp::Reverse(data.photos[*id].day));
        ids.truncate(RECENT_COUNT);
        let thumbs: Vec<Thumb> = ids
            .iter()
            .map(|id| {
                let photo = &data.photos[*id];
                let image = self.picture(photo);
                Thumb {
                    id: *id as i32,
                    has_image: image.is_some(),
                    image: image.unwrap_or_default(),
                    ratio: photo.width as f32 / photo.height as f32,
                }
            })
            .collect();
        *self.recent.borrow_mut() = ids;
        self.state().set_recent(ModelRc::new(VecModel::from(thumbs)));
    }

    fn album_card(&self, index: usize) -> Album {
        let live = self.live_photos();
        let data = self.data.borrow();
        let album = &data.albums[index];
        let ids: Vec<usize> = live.into_iter().filter(|id| data.photos[*id].albums.contains(&index)).collect();
        let (cover, has_cover) = self.cover_of(ids.iter().copied());
        Album {
            id: index as i32,
            name: album.name.clone().into(),
            count: plural(ids.len(), "изображение", "изображения", "изображений").into(),
            date: date_text(album.day).into(),
            description: album.description.clone().into(),
            cover,
            has_cover,
            favorite: album.favorite,
        }
    }

    fn refresh_albums(&self) {
        let (albums, groups) = {
            let data = self.data.borrow();
            let albums: Vec<(usize, Option<usize>, bool)> =
                data.albums.iter().enumerate().map(|(index, album)| (index, album.group, album.favorite)).collect();
            (albums, data.album_groups.clone())
        };
        let favorites: Vec<Album> =
            albums.iter().filter(|(_, _, favorite)| *favorite).map(|(index, _, _)| self.album_card(*index)).collect();
        let last_group = groups.len() - 1;
        let groups: Vec<AlbumGroup> = groups
            .iter()
            .enumerate()
            .map(|(group, name)| {
                let cards: Vec<Album> = albums
                    .iter()
                    .filter(|(_, album_group, _)| album_group.unwrap_or(last_group) == group)
                    .map(|(index, _, _)| self.album_card(*index))
                    .collect();
                AlbumGroup { name: name.clone().into(), albums: ModelRc::new(VecModel::from(cards)) }
            })
            .filter(|group| group.albums.row_count() > 0)
            .collect();
        let state = self.state();
        state.set_favorite_albums(ModelRc::new(VecModel::from(favorites)));
        state.set_album_groups(ModelRc::new(VecModel::from(groups)));
        state.set_current_album(self.album_card(self.album.get()));
    }

    fn refresh_tags_and_categories(&self) {
        let live = self.live_photos();
        let data = self.data.borrow();
        let tags: Vec<TagItem> = data
            .tags
            .iter()
            .enumerate()
            .map(|(index, name)| TagItem {
                name: name.clone().into(),
                count: live.iter().filter(|id| data.photos[**id].tags.contains(&index)).count().to_string().into(),
            })
            .collect();
        let categories: Vec<CategoryItem> = data
            .categories
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let ids: Vec<usize> = live.iter().copied().filter(|id| data.photos[*id].category == index).collect();
                let (cover, has_cover) = self.cover_of(ids.iter().copied());
                CategoryItem {
                    name: name.clone().into(),
                    count: plural(ids.len(), "изображение", "изображения", "изображений").into(),
                    cover,
                    has_cover,
                }
            })
            .collect();
        let state = self.state();
        state.set_tags(ModelRc::new(VecModel::from(tags)));
        state.set_categories(ModelRc::new(VecModel::from(categories)));
    }

    fn refresh_duplicates(&self) {
        let data = self.data.borrow();
        let library = self.state().get_library();
        let shown = |groups: &[mock::DuplicateGroup]| -> Vec<usize> {
            (0..groups.len())
                .filter(|index| library < 0 || data.photos[groups[*index].copies[0].photo].library == library as usize)
                .collect()
        };
        let exact = shown(&data.exact);
        let similar = shown(&data.similar);
        let to_ui = |group: &mock::DuplicateGroup, is_similar: bool| -> DupGroup {
            let files: Vec<DupFile> = group
                .copies
                .iter()
                .map(|copy| {
                    let image = self.picture(&data.photos[copy.photo]);
                    DupFile {
                        has_image: image.is_some(),
                        image: image.unwrap_or_default(),
                        name: copy.name.clone().into(),
                        resolution: format!("{} × {}", copy.width, copy.height).into(),
                        size: size_text(copy.bytes).into(),
                        format: copy.format.into(),
                        path: copy.path.clone().into(),
                    }
                })
                .collect();
            let first = &group.copies[0];
            let (title, note) = if is_similar {
                (format!("Похожие · {}", first.name), format!("похожесть {}%", group.similarity))
            } else {
                let extra: u64 = group.copies.iter().skip(1).map(|copy| copy.bytes).sum();
                (
                    format!("{} · {}", plural(group.copies.len(), "копия", "копии", "копий"), first.name),
                    format!("можно освободить {}", size_text(extra)),
                )
            };
            DupGroup {
                files: ModelRc::new(VecModel::from(files)),
                keep: group.keep as i32,
                title: title.into(),
                note: note.into(),
            }
        };
        let exact_ui: Vec<DupGroup> = exact.iter().map(|index| to_ui(&data.exact[*index], false)).collect();
        let similar_ui: Vec<DupGroup> = similar.iter().map(|index| to_ui(&data.similar[*index], true)).collect();
        let extra: u64 = exact
            .iter()
            .flat_map(|index| data.exact[*index].copies.iter().skip(1).map(|copy| copy.bytes))
            .sum();
        let state = self.state();
        state.set_exact_summary(
            if exact.is_empty() {
                String::new()
            } else {
                format!("{} · можно освободить {}", plural(exact.len(), "группа", "группы", "групп"), size_text(extra))
            }
            .into(),
        );
        state.set_similar_summary(
            if similar.is_empty() { String::new() } else { plural(similar.len(), "пара", "пары", "пар") }.into(),
        );
        state.set_exact_groups(ModelRc::new(VecModel::from(exact_ui)));
        state.set_similar_groups(ModelRc::new(VecModel::from(similar_ui)));
        *self.exact_shown.borrow_mut() = exact;
        *self.similar_shown.borrow_mut() = similar;
    }

    /// Libraries on the import page: all, or only the chosen one.
    fn import_shown(&self) -> Vec<usize> {
        let library = self.state().get_library();
        let count = self.data.borrow().libraries.len();
        (0..count).filter(|index| library < 0 || *index == library as usize).collect()
    }

    fn refresh_import(&self) {
        let shown = self.import_shown();
        let live = self.live_photos();
        let data = self.data.borrow();
        let to_model = |list: &[String]| {
            let items: Vec<SharedString> = list.iter().map(|item| item.clone().into()).collect();
            ModelRc::new(VecModel::from(items))
        };
        let libraries: Vec<ImportLibrary> = shown
            .iter()
            .map(|index| {
                let library = &data.libraries[*index];
                let count = live.iter().filter(|id| data.photos[**id].library == *index).count();
                ImportLibrary {
                    name: library.name.clone().into(),
                    count: plural(count, "изображение", "изображения", "изображений").into(),
                    sources: to_model(&library.sources),
                    excluded: to_model(&library.excluded),
                }
            })
            .collect();
        self.state().set_import_libraries(ModelRc::new(VecModel::from(libraries)));
    }

    // ---- Toast ----

    fn toast(&self, text: &str) {
        let state = self.state();
        state.set_toast(text.into());
        state.set_toast_shown(true);
        let window = self.window.as_weak();
        self.toast_timer.start(TimerMode::SingleShot, TOAST_TIME, move || {
            if let Some(window) = window.upgrade() {
                window.global::<AppState>().set_toast_shown(false);
            }
        });
    }
}
