//! Context menu of tiles, dropdown lists of the toolbar and their submenus.

use super::*;

impl App {
    pub(super) fn entry(icon: Image, text: &str) -> MenuEntry {
        MenuEntry { icon, text: text.into(), ..Default::default() }
    }

    pub(super) fn separator() -> MenuEntry {
        MenuEntry { separator: true, ..Default::default() }
    }

    pub(super) fn open_menu(&self, kind: &str, entries: Vec<(MenuEntry, Action)>, checks: bool, x: f32, y: f32, width: f32) {
        let state = self.state();
        let (entries, actions): (Vec<MenuEntry>, Vec<Action>) = entries.into_iter().unzip();
        *self.actions.borrow_mut() = actions;
        self.submenu.set(None);
        state.set_submenu_of(-1);
        state.set_menu_kind(kind.into());
        state.set_menu_entries(ModelRc::new(VecModel::from(entries)));
        state.set_menu_checks(checks);
        state.set_menu_x(x);
        state.set_menu_y(y);
        state.set_menu_width(width);
        state.set_menu_open(true);
    }

    pub(super) fn tile_context(&self, _kind: GalleryKind, id: usize, x: f32, y: f32) {
        if !self.selection.borrow().contains(&id) {
            let mut selection = self.selection.borrow_mut();
            selection.clear();
            selection.insert(id);
            drop(selection);
            self.refresh_selection();
        }
        self.anchor.set(Some(id));
        let icons = self.ui().global::<Icons>();
        let count = self.selection.borrow().len();
        let all_favorite = {
            let data = self.data.borrow();
            self.selection.borrow().iter().all(|id| data.photos[*id].favorite)
        };
        let submenu = |icon, text| MenuEntry { submenu: true, ..Self::entry(icon, text) };
        let mut entries = vec![
            (Self::entry(icons.get_eye(), "Быстрый просмотр"), Action::QuickView),
            (Self::entry(icons.get_external_link(), "Открыть в Pictora"), Action::Pictora),
            (Self::separator(), Action::None),
        ];
        entries.push((submenu(icons.get_album(), "Добавить в альбом"), Action::AddToAlbum));
        entries.push((submenu(icons.get_tag(), "Теги"), Action::Tags));
        entries.push((submenu(icons.get_shapes(), "Категория"), Action::Category));
        let favorite_text = if all_favorite { "Убрать из избранного" } else { "В избранное" };
        entries.push((Self::entry(icons.get_heart(), favorite_text), Action::Favorite));
        entries.push((Self::separator(), Action::None));
        if count == 1 {
            entries.push((Self::entry(icons.get_pencil(), "Переименовать"), Action::Rename));
        }
        entries.push((Self::entry(icons.get_folder_input(), "Переместить"), Action::Move));
        entries.push((Self::entry(icons.get_folder_open(), "Показать в Проводнике"), Action::Explorer));
        entries.push((Self::separator(), Action::None));
        let delete_text = if count == 1 { "Удалить".to_string() } else { format!("Удалить ({count})") };
        entries.push((MenuEntry { danger: true, ..Self::entry(icons.get_trash_2(), &delete_text) }, Action::Delete));
        let width = self.ui().global::<Theme>().get_sidebar_width();
        self.open_menu("tile", entries, false, x, y, width);
    }

    pub(super) fn show_menu(&self, kind: &str, x: f32, y: f32, width: f32) {
        let state = self.state();
        if state.get_menu_open() && state.get_menu_kind() == kind {
            state.set_menu_open(false);
            return;
        }
        let entries = self.dropdown_entries(kind);
        self.open_menu(kind, entries, true, x, y, width);
    }

    pub(super) fn dropdown_entries(&self, kind: &str) -> Vec<(MenuEntry, Action)> {
        let data = self.data.borrow();
        let live = self.live_photos();
        let checked = |entry: MenuEntry, on: bool| MenuEntry { checked: on, ..entry };
        match kind {
            "sort" => {
                let sort = self.state().get_sort();
                ["Имя", "Дата", "Размер", "Формат"]
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        (checked(Self::entry(Image::default(), name), sort == index as i32), Action::Sort(index as i32))
                    })
                    .collect()
            }
            "category-filter" => {
                let chosen = self.categories.borrow();
                let mut entries = vec![(checked(Self::entry(Image::default(), "Все категории"), chosen.is_empty()), Action::AllCategories)];
                entries.push((Self::separator(), Action::None));
                entries.extend(data.categories.iter().enumerate().map(|(index, name)| {
                    let count = live.iter().filter(|id| data.photos[**id].category == index).count();
                    let entry = MenuEntry { trailing: count.to_string().into(), ..Self::entry(Image::default(), name) };
                    (checked(entry, chosen.contains(&index)), Action::FilterCategory(index))
                }));
                entries
            }
            _ => {
                let chosen = self.tags.borrow();
                let mut entries = vec![(checked(Self::entry(Image::default(), "Все теги"), chosen.is_empty()), Action::AllTags)];
                entries.push((Self::separator(), Action::None));
                entries.extend(data.tags.iter().enumerate().map(|(index, name)| {
                    let count = live.iter().filter(|id| data.photos[**id].tags.contains(&index)).count();
                    let entry = MenuEntry { trailing: count.to_string().into(), ..Self::entry(Image::default(), name) };
                    (checked(entry, chosen.contains(&index)), Action::FilterTag(index))
                }));
                entries
            }
        }
    }

    /// Lists with check marks stay open after a click, so several can be picked.
    pub(super) fn refresh_dropdown(&self) {
        let state = self.state();
        let entries: Vec<MenuEntry> = self.dropdown_entries(&state.get_menu_kind()).into_iter().map(|(entry, _)| entry).collect();
        state.set_menu_entries(ModelRc::new(VecModel::from(entries)));
        self.refresh_views();
    }

    pub(super) fn menu_activated(&self, index: usize) {
        let Some(action) = self.actions.borrow().get(index).copied() else {
            return;
        };
        let state = self.state();
        let close = || state.set_menu_open(false);
        let selection: Vec<usize> = self.selection.borrow().iter().copied().collect();
        match action {
            Action::QuickView => {
                close();
                if let Some(id) = self.anchor.get() {
                    let kind = self.current_gallery();
                    self.quick_open(kind, id, None);
                }
            }
            Action::Pictora => {
                close();
                self.toast("Открыть в Pictora — в макете не работает");
            }
            Action::AddToAlbum | Action::Tags | Action::Category => self.open_submenu(index),
            Action::Favorite => {
                close();
                let mut data = self.data.borrow_mut();
                let make = !selection.iter().all(|id| data.photos[*id].favorite);
                for id in &selection {
                    data.photos[*id].favorite = make;
                }
                drop(data);
                self.refresh_views();
                self.toast(if make { "Добавлено в избранное" } else { "Убрано из избранного" });
            }
            Action::Rename => {
                close();
                let name = selection.first().map(|id| self.data.borrow().photos[*id].name.clone()).unwrap_or_default();
                self.show_dialog("rename", "Переименовать", "", "Переименовать", false, Some(&name));
            }
            Action::Move => {
                close();
                self.toast("Выбор папки для переноса — в макете заглушка");
            }
            Action::Explorer => {
                close();
                self.toast("Показать в Проводнике — в макете заглушка");
            }
            Action::Delete => {
                close();
                let count = selection.len();
                let title = format!("Удалить {}?", plural(count, "изображение", "изображения", "изображений"));
                let text = "Файлы будут перемещены в Корзину Windows — их можно будет восстановить.";
                self.show_dialog("delete", &title, text, "В Корзину", true, None);
            }
            Action::Sort(sort) => {
                close();
                state.set_sort(sort);
                self.refresh_views();
            }
            Action::AllCategories => {
                self.categories.borrow_mut().clear();
                self.refresh_dropdown();
            }
            Action::FilterCategory(category) => {
                toggle(&mut self.categories.borrow_mut(), category);
                self.refresh_dropdown();
            }
            Action::AllTags => {
                self.tags.borrow_mut().clear();
                self.refresh_dropdown();
            }
            Action::FilterTag(tag) => {
                toggle(&mut self.tags.borrow_mut(), tag);
                self.refresh_dropdown();
            }
            Action::None => {}
        }
    }

    pub(super) fn current_gallery(&self) -> GalleryKind {
        match self.state().get_page() {
            Page::Favorites => GalleryKind::Favorites,
            Page::Album => GalleryKind::Album,
            _ => GalleryKind::All,
        }
    }

    pub(super) fn open_submenu(&self, index: usize) {
        let submenu = match self.actions.borrow().get(index) {
            Some(Action::AddToAlbum) => Submenu::Albums,
            Some(Action::Tags) => Submenu::Tags,
            Some(Action::Category) => Submenu::Category,
            _ => return,
        };
        self.submenu.set(Some(submenu));
        self.refresh_submenu();
        self.state().set_submenu_of(index as i32);
    }

    pub(super) fn refresh_submenu(&self) {
        let Some(submenu) = self.submenu.get() else {
            return;
        };
        let data = self.data.borrow();
        let selection = self.selection.borrow();
        let all = |test: &dyn Fn(&Photo) -> bool| !selection.is_empty() && selection.iter().all(|id| test(&data.photos[*id]));
        let entry = |text: &str, checked: bool| MenuEntry { text: text.into(), checked, ..Default::default() };
        let mut entries: Vec<MenuEntry> = match submenu {
            Submenu::Albums => data
                .albums
                .iter()
                .enumerate()
                .map(|(index, album)| entry(&album.name, all(&|photo| photo.albums.contains(&index))))
                .collect(),
            Submenu::Tags => data
                .tags
                .iter()
                .enumerate()
                .map(|(index, tag)| entry(tag, all(&|photo| photo.tags.contains(&index))))
                .collect(),
            Submenu::Category => data
                .categories
                .iter()
                .enumerate()
                .map(|(index, category)| entry(category, all(&|photo| photo.category == index)))
                .collect(),
        };
        if submenu == Submenu::Albums {
            entries.push(Self::separator());
            entries.push(entry("Новый альбом…", false));
        }
        self.state().set_submenu_entries(ModelRc::new(VecModel::from(entries)));
    }

    pub(super) fn submenu_activated(&self, index: usize) {
        let Some(submenu) = self.submenu.get() else {
            return;
        };
        let state = self.state();
        let selection: Vec<usize> = self.selection.borrow().iter().copied().collect();
        let mut data = self.data.borrow_mut();
        match submenu {
            Submenu::Albums if index >= data.albums.len() => {
                drop(data);
                state.set_menu_open(false);
                self.show_dialog("album-with-selection", "Новый альбом", "", "Создать", false, Some(""));
                return;
            }
            Submenu::Albums => {
                let add = !selection.iter().all(|id| data.photos[*id].albums.contains(&index));
                for id in &selection {
                    let albums = &mut data.photos[*id].albums;
                    albums.retain(|album| *album != index);
                    if add {
                        albums.push(index);
                    }
                }
                let name = data.albums[index].name.clone();
                drop(data);
                state.set_menu_open(false);
                self.toast(&if add { format!("Добавлено в «{name}»") } else { format!("Убрано из «{name}»") });
                self.refresh_albums();
                self.refresh_view(GalleryKind::Album);
                return;
            }
            Submenu::Tags => {
                let add = !selection.iter().all(|id| data.photos[*id].tags.contains(&index));
                for id in &selection {
                    let tags = &mut data.photos[*id].tags;
                    tags.retain(|tag| *tag != index);
                    if add {
                        tags.push(index);
                    }
                }
            }
            Submenu::Category => {
                for id in &selection {
                    data.photos[*id].category = index;
                }
                drop(data);
                state.set_menu_open(false);
                self.refresh_tags_and_categories();
                self.refresh_views();
                return;
            }
        }
        drop(data);
        self.refresh_submenu();
        self.refresh_tags_and_categories();
        self.refresh_views();
    }
}

fn toggle(list: &mut Vec<usize>, item: usize) {
    if let Some(position) = list.iter().position(|x| *x == item) {
        list.remove(position);
    } else {
        list.push(item);
    }
}
