//! fv-notepad — простой текстовый редактор. RU по умолчанию, EN в настройках (⚙).

use gtk::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ru,
    En,
}

fn settings_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("fv-notepad");
    let _ = fs::create_dir_all(&dir);
    dir.push("settings.json");
    dir
}

fn load_lang() -> Lang {
    if let Ok(bytes) = fs::read(settings_path()) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if v.get("lang").and_then(|s| s.as_str()) == Some("en") {
                return Lang::En;
            }
        }
    }
    Lang::Ru
}

fn save_lang(lang: Lang) {
    let val = if lang == Lang::En { "en" } else { "ru" };
    let _ = fs::write(
        settings_path(),
        serde_json::json!({ "lang": val }).to_string(),
    );
}

fn tr(lang: Lang, ru: &str, en: &str) -> String {
    if lang == Lang::Ru {
        ru.to_string()
    } else {
        en.to_string()
    }
}

fn buffer_text(buffer: &gtk::TextBuffer) -> String {
    let (start, end) = (buffer.start_iter(), buffer.end_iter());
    buffer.text(&start, &end, false).to_string()
}

fn set_title(lang: Lang, window: &gtk::ApplicationWindow, path: Option<&Path>, modified: bool) {
    let name = path
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or(if lang == Lang::Ru {
            "Без названия"
        } else {
            "Untitled"
        });
    let star = if modified { " •" } else { "" };
    let app = tr(lang, "Блокнот", "Notepad");
    window.set_title(Some(&format!("{name}{star} — {app}")));
}

fn main() {
    let app = gtk::Application::new(Some("org.fvpack.notepad"), Default::default());
    app.connect_activate(build_ui);
    app.run();
}

fn open_settings(
    parent: &gtk::ApplicationWindow,
    lang: &Rc<RefCell<Lang>>,
    on_change: Rc<dyn Fn()>,
) {
    let dlg = gtk::Window::new();
    dlg.set_transient_for(Some(parent));
    dlg.set_modal(true);
    let cur = *lang.borrow();
    dlg.set_title(Some(&tr(cur, "Настройки", "Settings")));
    dlg.set_default_size(300, 140);
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_top(14);
    vbox.set_margin_bottom(14);
    vbox.set_margin_start(14);
    vbox.set_margin_end(14);
    dlg.set_child(Some(&vbox));
    let lbl = gtk::Label::new(Some(&tr(cur, "Язык:", "Language:")));
    lbl.set_halign(gtk::Align::Start);
    vbox.append(&lbl);
    let dd = gtk::DropDown::from_strings(&["Русский", "English"]);
    dd.set_selected(if cur == Lang::Ru { 0 } else { 1 });
    vbox.append(&dd);
    let close_btn = gtk::Button::with_label(&tr(cur, "Закрыть", "Close"));
    vbox.append(&close_btn);
    {
        let lang = lang.clone();
        let dlg = dlg.clone();
        dd.connect_selected_notify(move |d| {
            let nl = if d.selected() == 0 {
                Lang::Ru
            } else {
                Lang::En
            };
            *lang.borrow_mut() = nl;
            save_lang(nl);
            on_change();
            dlg.close();
        });
    }
    {
        let dlg = dlg.clone();
        close_btn.connect_clicked(move |_| dlg.close());
    }
    dlg.present();
}

fn build_ui(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(700, 500);

    let current_file: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let lang: Rc<RefCell<Lang>> = Rc::new(RefCell::new(load_lang()));

    let header = gtk::HeaderBar::new();
    window.set_titlebar(Some(&header));

    let new_btn = gtk::Button::with_label("");
    let open_btn = gtk::Button::with_label("");
    let save_btn = gtk::Button::with_label("");
    save_btn.add_css_class("suggested-action");
    let save_as_btn = gtk::Button::with_label("");
    let settings_btn = gtk::Button::with_label("⚙");
    settings_btn.set_tooltip_text(Some("Настройки / Settings"));
    header.pack_start(&new_btn);
    header.pack_start(&open_btn);
    header.pack_start(&save_btn);
    header.pack_start(&settings_btn);
    header.pack_end(&save_as_btn);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    vbox.append(&scrolled);

    let text_view = gtk::TextView::new();
    text_view.set_wrap_mode(gtk::WrapMode::WordChar);
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);
    scrolled.set_child(Some(&text_view));

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_margin_start(8);
    status.set_margin_end(8);
    status.set_margin_top(4);
    status.set_margin_bottom(4);
    status.add_css_class("dim-label");
    vbox.append(&status);

    let buffer = text_view.buffer();

    // apply language to static widgets + title + status
    let apply_lang: Rc<dyn Fn()> = {
        let lang = lang.clone();
        let window = window.clone();
        let current_file = current_file.clone();
        let status = status.clone();
        let buffer = buffer.clone();
        let new_btn = new_btn.clone();
        let open_btn = open_btn.clone();
        let save_btn = save_btn.clone();
        let save_as_btn = save_as_btn.clone();
        Rc::new(move || {
            let lg = *lang.borrow();
            new_btn.set_label(&tr(lg, "Новый", "New"));
            open_btn.set_label(&tr(lg, "Открыть", "Open"));
            save_btn.set_label(&tr(lg, "Сохранить", "Save"));
            save_as_btn.set_label(&tr(lg, "Сохранить как", "Save As"));
            set_title(
                lg,
                &window,
                current_file.borrow().as_deref(),
                buffer.is_modified(),
            );
            if !buffer.is_modified() {
                if current_file.borrow().is_none() {
                    status.set_text(&tr(lg, "Новый файл", "New file"));
                } else {
                    status.set_text(&tr(lg, "Сохранено", "Saved"));
                }
            }
        })
    };
    apply_lang();

    {
        let window = window.clone();
        let current_file = current_file.clone();
        let lang_m = lang.clone();
        let lang_c = lang.clone();
        let status_m = status.clone();
        let status2 = status.clone();
        let buffer_clone = buffer.clone();
        buffer.connect_modified_changed(move |b| {
            let lg = *lang_m.borrow();
            let modified = b.is_modified();
            set_title(lg, &window, current_file.borrow().as_deref(), modified);
            if modified {
                let chars = b.char_count();
                status_m.set_text(&format!(
                    "{} — {} {}",
                    tr(lg, "Изменён", "Modified"),
                    chars,
                    tr(lg, "симв.", "chars")
                ));
            }
        });
        buffer_clone.connect_changed(move |b| {
            let lg = *lang_c.borrow();
            if b.is_modified() {
                status2.set_text(&format!(
                    "{} — {} {}",
                    tr(lg, "Изменён", "Modified"),
                    b.char_count(),
                    tr(lg, "симв.", "chars")
                ));
            } else {
                status2.set_text(&tr(lg, "Сохранено", "Saved"));
            }
        });
    }

    {
        let window = window.clone();
        let lang = lang.clone();
        let apply_lang = apply_lang.clone();
        settings_btn.connect_clicked(move |_| {
            open_settings(&window, &lang, apply_lang.clone());
        });
    }

    {
        let buffer = buffer.clone();
        let current_file = current_file.clone();
        let window = window.clone();
        let status = status.clone();
        let lang = lang.clone();
        let apply_lang = apply_lang.clone();
        new_btn.connect_clicked(move |_| {
            buffer.set_text("");
            buffer.set_modified(false);
            *current_file.borrow_mut() = None;
            let lg = *lang.borrow();
            set_title(lg, &window, None, false);
            status.set_text(&tr(lg, "Новый файл", "New file"));
            let _ = &apply_lang;
        });
    }

    {
        let window = window.clone();
        let buffer = buffer.clone();
        let current_file = current_file.clone();
        let status = status.clone();
        let lang = lang.clone();
        open_btn.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::new();
            let lg = *lang.borrow();
            dialog.set_title(&tr(lg, "Открыть файл", "Open file"));
            let window_c = window.clone();
            let buffer_c = buffer.clone();
            let cf_c = current_file.clone();
            let status_c = status.clone();
            let lang_c = lang.clone();
            dialog.open(Some(&window), None::<&gtk::gio::Cancellable>, move |res| {
                let Ok(file) = res else { return };
                let Some(path) = file.path() else { return };
                let lg = *lang_c.borrow();
                match fs::read_to_string(&path) {
                    Ok(contents) => {
                        buffer_c.set_text(&contents);
                        buffer_c.set_modified(false);
                        *cf_c.borrow_mut() = Some(path.clone());
                        set_title(lg, &window_c, Some(&path), false);
                        status_c.set_text(&format!(
                            "{} {} ({} {})",
                            tr(lg, "Открыт", "Opened"),
                            path.display(),
                            buffer_c.char_count(),
                            tr(lg, "симв.", "chars")
                        ));
                    }
                    Err(e) => {
                        status_c.set_text(&format!(
                            "{}: {e}",
                            tr(lg, "Ошибка открытия", "Open failed")
                        ));
                    }
                }
            });
        });
    }

    let do_save_as: Rc<dyn Fn()> = {
        let window = window.clone();
        let buffer = buffer.clone();
        let current_file = current_file.clone();
        let status = status.clone();
        let lang = lang.clone();
        Rc::new(move || {
            let window_c = window.clone();
            let buffer_c = buffer.clone();
            let cf_c = current_file.clone();
            let status_c = status.clone();
            let lang_c = lang.clone();
            let lg = *lang.borrow();
            let dialog = gtk::FileDialog::new();
            dialog.set_title(&tr(lg, "Сохранить файл", "Save file"));
            if let Some(p) = current_file.borrow().as_ref() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    dialog.set_initial_name(Some(name));
                }
            } else {
                dialog.set_initial_name(Some("untitled.txt"));
            }
            dialog.save(Some(&window), None::<&gtk::gio::Cancellable>, move |res| {
                let Ok(file) = res else { return };
                let Some(path) = file.path() else { return };
                write_buffer_to(
                    *lang_c.borrow(),
                    &buffer_c,
                    &window_c,
                    &cf_c,
                    &status_c,
                    &path,
                );
            });
        })
    };

    {
        let buffer = buffer.clone();
        let window = window.clone();
        let current_file = current_file.clone();
        let status = status.clone();
        let lang = lang.clone();
        let do_save_as = do_save_as.clone();
        save_btn.connect_clicked(move |_| {
            if let Some(path) = current_file.borrow().clone() {
                write_buffer_to(
                    *lang.borrow(),
                    &buffer,
                    &window,
                    &current_file,
                    &status,
                    &path,
                );
            } else {
                do_save_as();
            }
        });
    }

    {
        let do_save_as = do_save_as.clone();
        save_as_btn.connect_clicked(move |_| do_save_as());
    }

    window.present();
}

fn write_buffer_to(
    lang: Lang,
    buffer: &gtk::TextBuffer,
    window: &gtk::ApplicationWindow,
    current_file: &Rc<RefCell<Option<PathBuf>>>,
    status: &gtk::Label,
    path: &Path,
) {
    let text = buffer_text(buffer);
    match fs::write(path, text) {
        Ok(()) => {
            buffer.set_modified(false);
            *current_file.borrow_mut() = Some(path.to_path_buf());
            set_title(lang, window, Some(path), false);
            status.set_text(&format!(
                "{} {}",
                tr(lang, "Сохранено в", "Saved to"),
                path.display()
            ));
        }
        Err(e) => {
            status.set_text(&format!(
                "{}: {e}",
                tr(lang, "Ошибка сохранения", "Save failed")
            ));
        }
    }
}
