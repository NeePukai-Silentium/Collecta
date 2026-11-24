//! The pretend scan on the import page: a timer moves the progress bar.

use super::*;

impl App {
    pub(super) fn scan_start(&self) {
        self.scan.borrow_mut().step = 0;
        let state = self.state();
        state.set_scan_finished(false);
        state.set_scan_progress(0.0);
        state.set_scanning(true);
        let me = self.later();
        self.scan_timer.start(TimerMode::Repeated, SCAN_STEP, move || {
            if let Some(app) = me.upgrade() {
                app.scan_tick();
            }
        });
    }

    pub(super) fn scan_tick(&self) {
        const STEPS: u32 = 100;
        let step = {
            let mut scan = self.scan.borrow_mut();
            scan.step += 1;
            scan.step
        };
        let state = self.state();
        let folders: Vec<String> = {
            let data = self.data.borrow();
            self.import_shown().iter().flat_map(|index| data.libraries[*index].sources.clone()).collect()
        };
        if let Some(folder) = folders.get((step as usize / 12) % folders.len().max(1)) {
            state.set_scan_folder(format!(r"{folder}\{}", ["2024", "2025", "Разное", "Экспорт"][step as usize % 4]).into());
        }
        state.set_scan_counter(format!("проверено {}", plural(step as usize * 37, "файл", "файла", "файлов")).into());
        state.set_scan_progress(step as f32 / STEPS as f32);
        if step >= STEPS {
            self.scan_stop(false);
            state.set_scan_finished(true);
            state.set_scan_result("Найдено новых: 37 · недоступных: 2".into());
            let missing: Vec<SharedString> = self.data.borrow().missing.iter().map(|file| file.clone().into()).collect();
            state.set_missing_files(ModelRc::new(VecModel::from(missing)));
        }
    }

    pub(super) fn scan_stop(&self, cancelled: bool) {
        self.scan_timer.stop();
        self.state().set_scanning(false);
        if cancelled {
            self.toast("Сканирование отменено");
        }
    }
}
