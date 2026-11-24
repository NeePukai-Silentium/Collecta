//! Navigation between pages, selection of tiles and quick view.

use super::*;

impl App {
    pub(super) fn navigate(&self, page: Page) {
        let state = self.state();
        if matches!(page, Page::Home | Page::All | Page::Albums | Page::Tags | Page::Categories | Page::Favorites)
            && state.get_library() >= 0
        {
            state.set_library(-1);
            self.refresh_library_dependent();
        }
        self.selection.borrow_mut().clear();
        self.refresh_selection();
        state.set_menu_open(false);
        state.set_page(page);
    }

    pub(super) fn select_library(&self, index: i32) {
        let state = self.state();
        state.set_library(index);
        self.selection.borrow_mut().clear();
        self.refresh_library_dependent();
        if index >= 0 {
            state.set_page(Page::All);
        }
    }

    pub(super) fn refresh_library_dependent(&self) {
        self.refresh_views();
        self.refresh_duplicates();
        self.refresh_import();
    }

    pub(super) fn open_album(&self, index: usize) {
        self.album.set(index);
        self.state().set_current_album(self.album_card(index));
        self.refresh_view(GalleryKind::Album);
        self.navigate(Page::Album);
    }

    pub(super) fn tag_clicked(&self, name: &str) {
        let Some(tag) = self.data.borrow().tags.iter().position(|tag| tag == name) else {
            return;
        };
        *self.tags.borrow_mut() = vec![tag];
        self.categories.borrow_mut().clear();
        self.navigate(Page::All);
        self.refresh_views();
    }

    pub(super) fn remove_filter(&self, index: usize) {
        let categories = self.categories.borrow().len();
        if index < categories {
            self.categories.borrow_mut().remove(index);
        } else if index - categories < self.tags.borrow().len() {
            self.tags.borrow_mut().remove(index - categories);
        }
        self.refresh_views();
    }

    // ---- Selection, quick view ----

    pub(super) fn ids_of(&self, kind: GalleryKind) -> Vec<usize> {
        match kind {
            GalleryKind::Recent => self.recent.borrow().clone(),
            _ => self.views.borrow()[index_of(kind)].ids.clone(),
        }
    }

    pub(super) fn tile_clicked(&self, kind: GalleryKind, id: usize, control: bool, shift: bool, rect: [f32; 4]) {
        if kind != GalleryKind::Recent {
            let mut selection = self.selection.borrow_mut();
            if control {
                if !selection.remove(&id) {
                    selection.insert(id);
                }
                self.anchor.set(Some(id));
            } else if shift {
                let ids = self.ids_of(kind);
                let anchor = self.anchor.get().and_then(|anchor| ids.iter().position(|x| *x == anchor));
                let target = ids.iter().position(|x| *x == id);
                if let (Some(a), Some(b)) = (anchor, target) {
                    selection.clear();
                    selection.extend(ids[a.min(b)..=a.max(b)].iter().copied());
                }
            } else {
                selection.clear();
                selection.insert(id);
                self.anchor.set(Some(id));
            }
            drop(selection);
            self.refresh_selection();
            if control || shift {
                return;
            }
        }

        let previous = self.pending.borrow_mut().take();
        if let Some(previous) = previous
            && previous.kind == kind
            && previous.id == id
            && previous.at.elapsed() < DOUBLE_CLICK
        {
            self.click_timer.stop();
            self.toast("Двойной клик — «Открыть в Pictora», в макете не работает");
            return;
        }
        *self.pending.borrow_mut() = Some(PendingClick { kind, id, at: Instant::now(), rect });
        let me = self.later();
        self.click_timer.start(TimerMode::SingleShot, DOUBLE_CLICK, move || {
            if let Some(app) = me.upgrade() {
                let pending = app.pending.borrow_mut().take();
                if let Some(click) = pending {
                    app.quick_open(click.kind, click.id, Some(click.rect));
                }
            }
        });
    }

    pub(super) fn quick_open_selected(&self, kind: GalleryKind) {
        let ids = self.ids_of(kind);
        let id = self.anchor.get().filter(|anchor| ids.contains(anchor)).or(ids.first().copied());
        if let Some(id) = id {
            self.quick_open(kind, id, None);
        }
    }

    pub(super) fn quick_open(&self, kind: GalleryKind, id: usize, rect: Option<[f32; 4]>) {
        let ids = self.ids_of(kind);
        let Some(position) = ids.iter().position(|x| *x == id) else {
            return;
        };
        let state = self.state();
        let [x, y, width, height] = rect.unwrap_or_default();
        state.set_quick_from_x(x);
        state.set_quick_from_y(y);
        state.set_quick_from_width(width);
        state.set_quick_from_height(height);
        *self.quick.borrow_mut() = Some(Quick { ids, position, generation: 0 });
        self.show_quick_photo();
        state.set_quick_open(true);
    }

    pub(super) fn show_quick_photo(&self) {
        let mut quick = self.quick.borrow_mut();
        let Some(quick) = quick.as_mut() else {
            return;
        };
        let id = quick.ids[quick.position];
        let data = self.data.borrow();
        let photo = &data.photos[id];
        let state = self.state();
        let image = self.picture(photo);
        state.set_quick_has_image(image.is_some());
        state.set_quick_image(image.unwrap_or_default());
        state.set_quick_ratio(photo.width as f32 / photo.height as f32);
        state.set_quick_counter(format!("{} из {}", quick.position + 1, quick.ids.len()).into());
        let names = |list: Vec<&String>| {
            let items: Vec<SharedString> = list.into_iter().map(|name| name.clone().into()).collect();
            ModelRc::new(VecModel::from(items))
        };
        state.set_quick_info(QuickInfo {
            name: photo.name.clone().into(),
            resolution: format!("{} × {}", photo.width, photo.height).into(),
            size: size_text(photo.bytes).into(),
            format: photo.format.into(),
            date: date_text(photo.day).into(),
            path: format!(r"{}\{}", photo.folder, photo.name).into(),
            library: data.libraries[photo.library].name.clone().into(),
            category: data.categories[photo.category].clone().into(),
            tags: names(photo.tags.iter().map(|tag| &data.tags[*tag]).collect()),
            albums: names(photo.albums.iter().map(|album| &data.albums[*album].name).collect()),
            favorite: photo.favorite,
        });
        state.set_quick_description(photo.description.clone().into());

        // The thumbnail shows at once; the picture at screen size replaces it when ready.
        let generation = self.quick_generation.get() + 1;
        self.quick_generation.set(generation);
        quick.generation = generation;
        if let Some(index) = photo.picture {
            let size = self.ui().window().size();
            let side = size.width.max(size.height).max(1);
            thumbs::spawn_full(self.picture_paths[index].clone(), side, move |pixels| {
                let _ = slint::invoke_from_event_loop(move || {
                    crate::with_app(|app| app.full_picture_loaded(generation, pixels));
                });
            });
        }
    }

    pub(super) fn full_picture_loaded(&self, generation: u64, pixels: Pixels) {
        let current = self.quick.borrow().as_ref().map(|quick| quick.generation);
        if current == Some(generation) {
            let state = self.state();
            state.set_quick_image(Image::from_rgba8(pixels));
            state.set_quick_has_image(true);
        }
    }

    pub(super) fn quick_step(&self, delta: i32) {
        {
            let mut quick = self.quick.borrow_mut();
            let Some(quick) = quick.as_mut() else {
                return;
            };
            let next = (quick.position as i32 + delta).clamp(0, quick.ids.len() as i32 - 1) as usize;
            if next == quick.position {
                return;
            }
            quick.position = next;
        }
        // Another photo: closing shrinks it in the middle instead of flying to the first tile.
        self.state().set_quick_from_width(0.0);
        self.show_quick_photo();
    }

    pub(super) fn quick_close(&self) {
        *self.quick.borrow_mut() = None;
        self.state().set_quick_open(false);
    }

    pub(super) fn quick_id(&self) -> Option<usize> {
        self.quick.borrow().as_ref().map(|quick| quick.ids[quick.position])
    }

    pub(super) fn quick_favorite(&self) {
        let Some(id) = self.quick_id() else {
            return;
        };
        {
            let mut data = self.data.borrow_mut();
            data.photos[id].favorite = !data.photos[id].favorite;
        }
        let state = self.state();
        let favorite = self.data.borrow().photos[id].favorite;
        state.set_quick_info(QuickInfo { favorite, ..state.get_quick_info() });
        self.refresh_views();
    }

    pub(super) fn quick_description(&self, text: SharedString) {
        if let Some(id) = self.quick_id() {
            self.data.borrow_mut().photos[id].description = text.into();
        }
    }
}
