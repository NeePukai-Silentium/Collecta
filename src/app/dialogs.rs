//! Confirmations and name prompts, libraries and their folders, duplicates.

use super::*;

impl App {
    pub(super) fn show_dialog(&self, kind: &str, title: &str, text: &str, confirm: &str, danger: bool, input: Option<&str>) {
        let state = self.state();
        state.set_dialog_title(title.into());
        state.set_dialog_text(text.into());
        state.set_dialog_confirm(confirm.into());
        state.set_dialog_danger(danger);
        state.set_dialog_has_input(input.is_some());
        state.set_dialog_input(input.unwrap_or_default().into());
        state.set_dialog(kind.into());
    }

    pub(super) fn ask_name(&self, kind: &str) {
        match kind {
            "album" => self.show_dialog("album", "Новый альбом", "", "Создать", false, Some("")),
            "tag" => self.show_dialog("tag", "Новый тег", "", "Создать", false, Some("")),
            _ => self.show_dialog("category", "Новая категория", "", "Создать", false, Some("")),
        }
    }

    pub(super) fn ask_dup_remove(&self, similar: bool, group: i32) {
        let data = self.data.borrow();
        let (groups, shown) = if similar { (&data.similar, self.similar_shown.borrow()) } else { (&data.exact, self.exact_shown.borrow()) };
        let chosen: Vec<usize> = if group < 0 { shown.clone() } else { shown.get(group as usize).copied().into_iter().collect() };
        let count: usize = chosen.iter().map(|index| groups[*index].copies.len() - 1).sum();
        drop(shown);
        drop(data);
        self.dialog_target.set((similar, group));
        let title = format!("Удалить {}?", plural(count, "лишнюю копию", "лишние копии", "лишних копий"));
        let text = "Файлы будут перемещены в Корзину Windows — их можно будет восстановить. Выбранные оригиналы останутся на месте.";
        self.show_dialog("dup-delete", &title, text, "В Корзину", true, None);
    }

    pub(super) fn ask_library_remove(&self, shown_index: usize) {
        let Some(index) = self.import_shown().get(shown_index).copied() else {
            return;
        };
        self.dialog_target.set((false, index as i32));
        let name = self.data.borrow().libraries[index].name.clone();
        let text = "Collecta забудет эту библиотеку и её альбомные отметки. Сами файлы останутся в своих папках.";
        self.show_dialog("library-remove", &format!("Удалить библиотеку «{name}»?"), text, "Удалить", true, None);
    }

    pub(super) fn dialog_confirmed(&self) {
        let state = self.state();
        let kind = state.get_dialog();
        let input = state.get_dialog_input().trim().to_string();
        let selection: Vec<usize> = self.selection.borrow().iter().copied().collect();
        match kind.as_str() {
            "delete" => {
                self.removed.borrow_mut().extend(selection.iter().copied());
                self.selection.borrow_mut().clear();
                self.refresh_all();
                self.toast(&format!("В Корзину: {}", plural(selection.len(), "изображение", "изображения", "изображений")));
            }
            "rename" if !input.is_empty() => {
                if let Some(id) = selection.first() {
                    self.data.borrow_mut().photos[*id].name = input;
                    self.refresh_views();
                }
            }
            "album" | "album-with-selection" if !input.is_empty() => {
                let index = {
                    let mut data = self.data.borrow_mut();
                    data.albums.push(mock::Album { name: input.clone(), group: None, day: 1420, description: String::new(), favorite: false });
                    data.albums.len() - 1
                };
                if kind == "album-with-selection" {
                    let mut data = self.data.borrow_mut();
                    for id in &selection {
                        data.photos[*id].albums.push(index);
                    }
                }
                self.refresh_albums();
                self.toast(&format!("Альбом «{input}» создан"));
            }
            "tag" if !input.is_empty() => {
                self.data.borrow_mut().tags.push(input);
                self.refresh_tags_and_categories();
            }
            "category" if !input.is_empty() => {
                self.data.borrow_mut().categories.push(input);
                self.refresh_tags_and_categories();
            }
            "dup-delete" => {
                let (similar, group) = self.dialog_target.get();
                let shown = if similar { self.similar_shown.borrow().clone() } else { self.exact_shown.borrow().clone() };
                let mut chosen: Vec<usize> = if group < 0 { shown } else { shown.get(group as usize).copied().into_iter().collect() };
                chosen.sort_unstable_by(|a, b| b.cmp(a));
                let mut data = self.data.borrow_mut();
                let groups = if similar { &mut data.similar } else { &mut data.exact };
                for index in chosen {
                    groups.remove(index);
                }
                drop(data);
                self.refresh_duplicates();
                self.toast("Лишние копии перемещены в Корзину");
            }
            "library-remove" => {
                let index = self.dialog_target.get().1 as usize;
                self.remove_library(index);
            }
            "add-library" => {
                let name = state.get_new_library_name().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let (sources, excluded) = std::mem::take(&mut *self.new_library.borrow_mut());
                self.data.borrow_mut().libraries.push(mock::Library { name: name.clone(), sources, excluded, scanned: "ещё не сканировалась" });
                self.refresh_all();
                self.toast(&format!("Библиотека «{name}» добавлена"));
            }
            _ => return,
        }
        state.set_dialog("".into());
    }

    pub(super) fn remove_library(&self, index: usize) {
        {
            let mut data = self.data.borrow_mut();
            data.libraries.remove(index);
            let mut removed = self.removed.borrow_mut();
            for (id, photo) in data.photos.iter_mut().enumerate() {
                if photo.library == index {
                    removed.insert(id);
                    photo.library = 0;
                } else if photo.library > index {
                    photo.library -= 1;
                }
            }
        }
        let state = self.state();
        state.set_library(-1);
        self.refresh_all();
        self.toast("Библиотека удалена");
    }

    pub(super) fn add_library(&self) {
        *self.new_library.borrow_mut() = (Vec::new(), Vec::new());
        let state = self.state();
        state.set_new_library_name("".into());
        self.show_new_library_folders();
        state.set_dialog("add-library".into());
    }

    pub(super) fn mock_folder(&self) -> String {
        let number = self.folder_counter.get();
        self.folder_counter.set(number + 1);
        format!(r"D:\Новая папка {number}")
    }

    pub(super) fn new_library_folder_add(&self, excluded: bool) {
        let folder = self.mock_folder();
        let mut lists = self.new_library.borrow_mut();
        if excluded { lists.1.push(folder) } else { lists.0.push(folder) }
        drop(lists);
        self.show_new_library_folders();
    }

    pub(super) fn show_new_library_folders(&self) {
        let lists = self.new_library.borrow();
        let to_model = |list: &[String]| {
            let items: Vec<SharedString> = list.iter().map(|item| item.clone().into()).collect();
            ModelRc::new(VecModel::from(items))
        };
        let state = self.state();
        state.set_new_library_sources(to_model(&lists.0));
        state.set_new_library_excluded(to_model(&lists.1));
    }

    pub(super) fn folder_add(&self, shown_index: usize, excluded: bool) {
        let Some(index) = self.import_shown().get(shown_index).copied() else {
            return;
        };
        let folder = self.mock_folder();
        let mut data = self.data.borrow_mut();
        let library = &mut data.libraries[index];
        if excluded { library.excluded.push(folder) } else { library.sources.push(folder) }
        drop(data);
        self.refresh_import();
        self.refresh_library_cards();
    }

    pub(super) fn folder_remove(&self, shown_index: usize, excluded: bool, folder: usize) {
        let Some(index) = self.import_shown().get(shown_index).copied() else {
            return;
        };
        let mut data = self.data.borrow_mut();
        let library = &mut data.libraries[index];
        let list = if excluded { &mut library.excluded } else { &mut library.sources };
        if folder < list.len() {
            list.remove(folder);
        }
        drop(data);
        self.refresh_import();
        self.refresh_library_cards();
    }

    pub(super) fn dup_keep(&self, similar: bool, shown_group: usize, file: usize) {
        {
            let shown = if similar { self.similar_shown.borrow() } else { self.exact_shown.borrow() };
            let Some(index) = shown.get(shown_group).copied() else {
                return;
            };
            drop(shown);
            let mut data = self.data.borrow_mut();
            let groups = if similar { &mut data.similar } else { &mut data.exact };
            groups[index].keep = file;
        }
        self.refresh_duplicates();
    }
}
