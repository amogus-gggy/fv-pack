//! fv-passman — простой менеджер паролей. RU по умолчанию, EN в настройках (⚙).

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gtk::prelude::*;
use rand::{distributions::Uniform, thread_rng, Rng, RngCore};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ru,
    En,
}

fn settings_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("fv-passman");
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

fn tr_vault_err(lang: Lang, e: &str) -> String {
    if lang == Lang::En {
        return e.to_string();
    }
    match e {
        "vault file is corrupt" => "файл хранилища повреждён".into(),
        "vault corrupt (salt)" => "хранилище повреждено (salt)".into(),
        "vault corrupt (nonce)" => "хранилище повреждено (nonce)".into(),
        "vault corrupt (data)" => "хранилище повреждено (data)".into(),
        "wrong master password or corrupt vault" => {
            "неверный мастер-пароль или хранилище повреждено".into()
        }
        "vault data corrupt" => "данные хранилища повреждены".into(),
        s if s.starts_with("read failed:") => "ошибка чтения хранилища".into(),
        s if s.starts_with("write failed:") => "ошибка записи хранилища".into(),
        s if s.starts_with("key derivation failed:") => "ошибка ключа".into(),
        s if s.starts_with("encrypt failed:") => "ошибка шифрования".into(),
        s if s.starts_with("serialize failed:") => "ошибка данных".into(),
        _ => e.to_string(),
    }
}

// ---------- vault model ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Entry {
    service: String,
    username: String,
    password: String,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultFile {
    salt: String,
    nonce: String,
    data: String,
}

fn vault_path() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("fv-passman");
    let _ = fs::create_dir_all(&dir);
    dir.push("vault.json");
    dir
}

fn vault_exists() -> bool {
    vault_path().exists()
}

fn derive_key(master: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(master.as_bytes(), salt, &mut key)
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(key)
}

fn encrypt_to_file(master: &str, entries: &[Entry]) -> Result<(), String> {
    let mut salt = [0u8; 16];
    thread_rng().fill_bytes(&mut salt);
    let key = derive_key(master, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let plaintext = serde_json::to_vec(entries).map_err(|e| format!("serialize failed: {e}"))?;
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_slice())
        .map_err(|e| format!("encrypt failed: {e}"))?;
    let file = VaultFile {
        salt: B64.encode(salt),
        nonce: B64.encode(nonce),
        data: B64.encode(ciphertext),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    fs::write(vault_path(), json).map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

fn load_entries(master: &str) -> Result<Vec<Entry>, String> {
    let raw = fs::read(vault_path()).map_err(|e| format!("read failed: {e}"))?;
    let file: VaultFile =
        serde_json::from_slice(&raw).map_err(|_| "vault file is corrupt".to_string())?;
    let salt = B64.decode(&file.salt).map_err(|_| "vault corrupt (salt)")?;
    let nonce_bytes = B64
        .decode(&file.nonce)
        .map_err(|_| "vault corrupt (nonce)")?;
    let ct = B64.decode(&file.data).map_err(|_| "vault corrupt (data)")?;
    let key = derive_key(master, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let pt = cipher
        .decrypt(nonce, ct.as_slice())
        .map_err(|_| "wrong master password or corrupt vault".to_string())?;
    let entries: Vec<Entry> =
        serde_json::from_slice(&pt).map_err(|_| "vault data corrupt".to_string())?;
    Ok(entries)
}

fn persist(
    lang: Lang,
    master: &Rc<RefCell<Option<String>>>,
    entries: &Rc<RefCell<Vec<Entry>>>,
    status: &gtk::Label,
) -> bool {
    let Some(pw) = master.borrow().clone() else {
        status.set_text(&tr(lang, "Не разблокировано.", "Not unlocked."));
        return false;
    };
    match encrypt_to_file(&pw, &entries.borrow()) {
        Ok(()) => true,
        Err(e) => {
            status.set_text(&format!(
                "{}: {}",
                tr(lang, "Ошибка сохранения", "Save failed"),
                tr_vault_err(lang, &e)
            ));
            false
        }
    }
}

fn generate_password(len: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghjkmnpqrstuvwxyzABCDEFGHJKMNPQRSTUVWXYZ23456789!@#$%^&*-_+=";
    let mut rng = thread_rng();
    let range = Uniform::from(0..CHARSET.len());
    (0..len)
        .map(|_| CHARSET[rng.sample(range)] as char)
        .collect()
}

// ---------- UI ----------

fn main() {
    let app = gtk::Application::new(Some("org.fvpack.passman"), Default::default());
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
    let lang: Rc<RefCell<Lang>> = Rc::new(RefCell::new(load_lang()));

    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(640, 480);

    let header = gtk::HeaderBar::new();
    let header_title = gtk::Label::new(None);
    header.set_title_widget(Some(&header_title));
    window.set_titlebar(Some(&header));

    let lock_btn = gtk::Button::with_label("");
    lock_btn.set_visible(false);
    header.pack_end(&lock_btn);
    let settings_btn = gtk::Button::with_label("⚙");
    settings_btn.set_tooltip_text(Some("Настройки / Settings"));
    header.pack_end(&settings_btn);

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    window.set_child(Some(&stack));

    let master: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let entries: Rc<RefCell<Vec<Entry>>> = Rc::new(RefCell::new(Vec::new()));

    // ----- auth page -----
    let auth_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    auth_box.set_margin_top(40);
    auth_box.set_margin_bottom(40);
    auth_box.set_margin_start(60);
    auth_box.set_margin_end(60);
    auth_box.set_halign(gtk::Align::Center);
    auth_box.set_valign(gtk::Align::Center);
    stack.add_named(&auth_box, Some("auth"));

    let auth_title = gtk::Label::new(None);
    auth_title.add_css_class("title-1");
    auth_box.append(&auth_title);

    let auth_sub = gtk::Label::new(None);
    auth_sub.add_css_class("dim-label");
    auth_box.append(&auth_sub);

    let pw1 = gtk::PasswordEntry::new();
    auth_box.append(&pw1);

    let pw2 = gtk::PasswordEntry::new();
    auth_box.append(&pw2);

    let auth_error = gtk::Label::new(None);
    auth_error.add_css_class("error");
    auth_box.append(&auth_error);

    let auth_btn = gtk::Button::with_label("");
    auth_btn.add_css_class("suggested-action");
    auth_box.append(&auth_btn);

    // ----- main page -----
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    main_box.set_margin_top(12);
    main_box.set_margin_bottom(12);
    main_box.set_margin_start(12);
    main_box.set_margin_end(12);
    stack.add_named(&main_box, Some("main"));

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    main_box.append(&toolbar);

    let search = gtk::SearchEntry::new();
    search.set_hexpand(true);
    toolbar.append(&search);

    let add_btn = gtk::Button::with_label("");
    add_btn.add_css_class("suggested-action");
    toolbar.append(&add_btn);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    main_box.append(&scrolled);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    scrolled.set_child(Some(&list));

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.add_css_class("dim-label");
    main_box.append(&status);

    // show correct auth labels depending on vault existence + lang
    let update_auth_labels = {
        let auth_title = auth_title.clone();
        let auth_sub = auth_sub.clone();
        let pw1 = pw1.clone();
        let pw2 = pw2.clone();
        let auth_btn = auth_btn.clone();
        let lang = lang.clone();
        move || {
            let lg = *lang.borrow();
            pw1.set_placeholder_text(Some(&tr(lg, "Мастер-пароль", "Master password")));
            pw2.set_placeholder_text(Some(&tr(
                lg,
                "Подтвердите мастер-пароль",
                "Confirm master password",
            )));
            if vault_exists() {
                auth_title.set_text(&tr(lg, "Разблокировать хранилище", "Unlock vault"));
                auth_sub.set_text(&tr(
                    lg,
                    "Введите мастер-пароль для разблокировки.",
                    "Enter your master password to unlock.",
                ));
                pw2.set_visible(false);
                auth_btn.set_label(&tr(lg, "Разблокировать", "Unlock"));
            } else {
                auth_title.set_text(&tr(lg, "Создать хранилище", "Create vault"));
                auth_sub.set_text(&tr(
                    lg,
                    "Придумайте надёжный мастер-пароль. Он не восстанавливается.",
                    "Choose a strong master password. It cannot be recovered.",
                ));
                pw2.set_visible(true);
                auth_btn.set_label(&tr(lg, "Создать", "Create"));
            }
        }
    };

    // static texts
    let apply_static: Rc<dyn Fn()> = {
        let lang = lang.clone();
        let window = window.clone();
        let header_title = header_title.clone();
        let lock_btn = lock_btn.clone();
        let search = search.clone();
        let add_btn = add_btn.clone();
        let update_auth_labels = update_auth_labels.clone();
        Rc::new(move || {
            let lg = *lang.borrow();
            window.set_title(Some(&tr(lg, "Менеджер паролей", "Password Manager")));
            header_title.set_text(&tr(lg, "Менеджер паролей", "Password Manager"));
            lock_btn.set_label(&tr(lg, "Заблокировать", "Lock"));
            search.set_placeholder_text(Some(&tr(
                lg,
                "Поиск по сервису или логину…",
                "Search service or username…",
            )));
            add_btn.set_label(&tr(lg, "+ Добавить", "+ Add"));
            update_auth_labels();
        })
    };
    apply_static();

    // ----- refresh list -----
    let refresh_cell: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let refresh: Rc<dyn Fn()> = {
        let list = list.clone();
        let entries = entries.clone();
        let search = search.clone();
        let status = status.clone();
        let window = window.clone();
        let master = master.clone();
        let lang = lang.clone();
        let refresh_cell = refresh_cell.clone();
        Rc::new(move || {
            let lg = *lang.borrow();
            while let Some(row) = list.first_child() {
                list.remove(&row);
            }
            let q = search.text().to_string().to_lowercase();
            let snapshot = entries.borrow().clone();
            let mut shown = 0usize;
            for (idx, e) in snapshot.iter().enumerate() {
                if !q.is_empty()
                    && !e.service.to_lowercase().contains(&q)
                    && !e.username.to_lowercase().contains(&q)
                {
                    continue;
                }
                shown += 1;

                let row = gtk::ListBoxRow::new();
                let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                hbox.set_margin_top(6);
                hbox.set_margin_bottom(6);
                hbox.set_margin_start(8);
                hbox.set_margin_end(8);
                row.set_child(Some(&hbox));

                let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
                vbox.set_hexpand(true);
                hbox.append(&vbox);

                let svc = gtk::Label::new(Some(&e.service));
                svc.set_halign(gtk::Align::Start);
                vbox.append(&svc);

                let user = gtk::Label::new(Some(&e.username));
                user.set_halign(gtk::Align::Start);
                user.add_css_class("dim-label");
                vbox.append(&user);

                let pwd_lbl = gtk::Label::new(Some("••••••••"));
                pwd_lbl.set_halign(gtk::Align::Start);
                pwd_lbl.add_css_class("monospace");
                vbox.append(&pwd_lbl);

                if !e.notes.is_empty() {
                    let notes = gtk::Label::new(Some(&e.notes));
                    notes.set_halign(gtk::Align::Start);
                    notes.add_css_class("dim-label");
                    vbox.append(&notes);
                }

                let show_btn = gtk::ToggleButton::with_label(&tr(lg, "Показать", "Show"));
                let copy_btn = gtk::Button::with_label(&tr(lg, "Копировать", "Copy"));
                let edit_btn = gtk::Button::with_label(&tr(lg, "Изменить", "Edit"));
                let del_btn = gtk::Button::with_label(&tr(lg, "Удалить", "Delete"));
                del_btn.add_css_class("destructive-action");
                hbox.append(&show_btn);
                hbox.append(&copy_btn);
                hbox.append(&edit_btn);
                hbox.append(&del_btn);

                {
                    let pwd_lbl = pwd_lbl.clone();
                    let real = e.password.clone();
                    show_btn.connect_toggled(move |b| {
                        if b.is_active() {
                            pwd_lbl.set_text(&real);
                        } else {
                            pwd_lbl.set_text("••••••••");
                        }
                    });
                }
                {
                    let real = e.password.clone();
                    let window = window.clone();
                    let status = status.clone();
                    let lg = lg;
                    copy_btn.connect_clicked(move |_| {
                        window.clipboard().set_text(&real);
                        status.set_text(&tr(
                            lg,
                            "Пароль скопирован в буфер обмена.",
                            "Password copied to clipboard.",
                        ));
                    });
                }
                {
                    let window = window.clone();
                    let entries = entries.clone();
                    let master = master.clone();
                    let status = status.clone();
                    let lang = lang.clone();
                    let cur = e.clone();
                    let refresh_cell = refresh_cell.clone();
                    edit_btn.connect_clicked(move |_| {
                        let recall = {
                            let refresh_cell = refresh_cell.clone();
                            Rc::new(move || {
                                if let Some(r) = refresh_cell.borrow().clone() {
                                    r();
                                }
                            })
                        };
                        open_entry_dialog(
                            *lang.borrow(),
                            &window,
                            Some((idx, cur.clone())),
                            &entries,
                            &master,
                            &status,
                            recall,
                        );
                    });
                }
                {
                    let entries = entries.clone();
                    let master = master.clone();
                    let status = status.clone();
                    let lang = lang.clone();
                    let refresh_cell = refresh_cell.clone();
                    let svc_name = e.service.clone();
                    let snap_entry = snapshot[idx].clone();
                    del_btn.connect_clicked(move |_| {
                        let lg = *lang.borrow();
                        let mut vec = entries.borrow_mut();
                        let pos = vec.iter().position(|x| {
                            x.service == snap_entry.service
                                && x.username == snap_entry.username
                                && x.password == snap_entry.password
                        });
                        if let Some(p) = pos {
                            vec.remove(p);
                        }
                        drop(vec);
                        let ok = persist(lg, &master, &entries, &status);
                        if ok {
                            status.set_text(&format!(
                                "{} '{svc_name}'.",
                                tr(lg, "Удалено", "Deleted")
                            ));
                        }
                        if let Some(r) = refresh_cell.borrow().clone() {
                            r();
                        }
                    });
                }

                list.append(&row);
            }
            status.set_text(&format!(
                "{} {} {} {}",
                shown,
                tr(lg, "из", "of"),
                snapshot.len(),
                tr(lg, "записей", "logins")
            ));
        })
    };
    *refresh_cell.borrow_mut() = Some(refresh.clone());

    // settings button: refresh static + list + auth labels
    {
        let window = window.clone();
        let lang = lang.clone();
        let apply_static = apply_static.clone();
        let refresh = refresh.clone();
        settings_btn.connect_clicked(move |_| {
            let apply_static = apply_static.clone();
            let refresh = refresh.clone();
            let combined: Rc<dyn Fn()> = Rc::new(move || {
                apply_static();
                refresh();
            });
            open_settings(&window, &lang, combined);
        });
    }

    {
        let refresh = refresh.clone();
        search.connect_search_changed(move |_| refresh());
    }

    {
        let window = window.clone();
        let entries = entries.clone();
        let master = master.clone();
        let status = status.clone();
        let lang = lang.clone();
        let refresh = refresh.clone();
        add_btn.connect_clicked(move |_| {
            open_entry_dialog(
                *lang.borrow(),
                &window,
                None,
                &entries,
                &master,
                &status,
                refresh.clone(),
            );
        });
    }

    // auth submit
    {
        let stack = stack.clone();
        let pw1_inner = pw1.clone();
        let pw2_inner = pw2.clone();
        let pw1_activate = pw1.clone();
        let pw2_activate = pw2.clone();
        let auth_error = auth_error.clone();
        let auth_btn = auth_btn.clone();
        let master = master.clone();
        let entries = entries.clone();
        let lock_btn = lock_btn.clone();
        let refresh = refresh.clone();
        let search = search.clone();
        let lang = lang.clone();
        auth_btn.connect_clicked(move |_| {
            let lg = *lang.borrow();
            let p1 = pw1_inner.text().to_string();
            if p1.len() < 4 {
                auth_error.set_text(&tr(
                    lg,
                    "Мастер-пароль должен быть не короче 4 символов.",
                    "Master password must be at least 4 characters.",
                ));
                return;
            }
            if vault_exists() {
                match load_entries(&p1) {
                    Ok(items) => {
                        *master.borrow_mut() = Some(p1);
                        *entries.borrow_mut() = items;
                        pw1_inner.set_text("");
                        auth_error.set_text("");
                        search.set_text("");
                        refresh();
                        stack.set_visible_child_name("main");
                        lock_btn.set_visible(true);
                    }
                    Err(e) => auth_error.set_text(&tr_vault_err(lg, &e)),
                }
            } else {
                let p2 = pw2_inner.text().to_string();
                if p1 != p2 {
                    auth_error.set_text(&tr(lg, "Пароли не совпадают.", "Passwords do not match."));
                    return;
                }
                match encrypt_to_file(&p1, &[]) {
                    Ok(()) => {
                        *master.borrow_mut() = Some(p1);
                        *entries.borrow_mut() = Vec::new();
                        pw1_inner.set_text("");
                        pw2_inner.set_text("");
                        auth_error.set_text("");
                        refresh();
                        stack.set_visible_child_name("main");
                        lock_btn.set_visible(true);
                    }
                    Err(e) => auth_error.set_text(&tr_vault_err(lg, &e)),
                }
            }
        });
        let b = auth_btn.clone();
        pw1_activate.connect_activate(move |_| b.emit_clicked());
        let b2 = auth_btn.clone();
        pw2_activate.connect_activate(move |_| b2.emit_clicked());
    }

    // lock
    {
        let stack = stack.clone();
        let master = master.clone();
        let entries = entries.clone();
        let pw1 = pw1.clone();
        let pw2 = pw2.clone();
        let auth_error = auth_error.clone();
        let lock_btn_c = lock_btn.clone();
        let update_auth_labels = update_auth_labels.clone();
        let lang = lang.clone();
        let status = status.clone();
        lock_btn.connect_clicked(move |_| {
            *master.borrow_mut() = None;
            *entries.borrow_mut() = Vec::new();
            pw1.set_text("");
            pw2.set_text("");
            auth_error.set_text("");
            update_auth_labels();
            let lg = *lang.borrow();
            status.set_text(&tr(lg, "Заблокировано", "Locked"));
            stack.set_visible_child_name("auth");
            lock_btn_c.set_visible(false);
        });
    }

    status.set_text(&tr(*lang.borrow(), "Заблокировано", "Locked"));
    stack.set_visible_child_name("auth");
    window.present();
}

#[allow(clippy::too_many_arguments)]
fn open_entry_dialog(
    lang: Lang,
    parent: &gtk::ApplicationWindow,
    existing: Option<(usize, Entry)>,
    entries: &Rc<RefCell<Vec<Entry>>>,
    master: &Rc<RefCell<Option<String>>>,
    status: &gtk::Label,
    on_done: Rc<dyn Fn()>,
) {
    let dlg = gtk::Window::new();
    dlg.set_transient_for(Some(parent));
    dlg.set_modal(true);
    let dlg_title = if existing.is_some() {
        tr(lang, "Изменить запись", "Edit login")
    } else {
        tr(lang, "Добавить запись", "Add login")
    };
    dlg.set_title(Some(&dlg_title));
    dlg.set_default_size(380, 300);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    dlg.set_child(Some(&vbox));

    let svc_entry = gtk::Entry::new();
    svc_entry.set_placeholder_text(Some(&tr(
        lang,
        "Сервис (напр. github.com)",
        "Service (e.g. github.com)",
    )));
    let user_entry = gtk::Entry::new();
    user_entry.set_placeholder_text(Some(&tr(lang, "Логин / email", "Username / email")));
    let pass_entry = gtk::PasswordEntry::new();
    pass_entry.set_placeholder_text(Some(&tr(lang, "Пароль", "Password")));
    pass_entry.set_show_peek_icon(true);
    let notes_entry = gtk::Entry::new();
    notes_entry.set_placeholder_text(Some(&tr(
        lang,
        "Заметки (необязательно)",
        "Notes (optional)",
    )));

    if let Some((_, ref e)) = existing {
        svc_entry.set_text(&e.service);
        user_entry.set_text(&e.username);
        pass_entry.set_text(&e.password);
        notes_entry.set_text(&e.notes);
    }

    vbox.append(&svc_entry);
    vbox.append(&user_entry);

    let pass_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    pass_entry.set_hexpand(true);
    pass_row.append(&pass_entry);
    let gen_btn = gtk::Button::with_label(&tr(lang, "Сгенерировать", "Generate"));
    pass_row.append(&gen_btn);
    vbox.append(&pass_row);

    vbox.append(&notes_entry);

    let err = gtk::Label::new(None);
    err.add_css_class("error");
    vbox.append(&err);

    let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    btn_row.set_halign(gtk::Align::End);
    vbox.append(&btn_row);
    let cancel_btn = gtk::Button::with_label(&tr(lang, "Отмена", "Cancel"));
    let save_btn = gtk::Button::with_label(&tr(lang, "Сохранить", "Save"));
    save_btn.add_css_class("suggested-action");
    btn_row.append(&cancel_btn);
    btn_row.append(&save_btn);

    {
        let pass_entry = pass_entry.clone();
        gen_btn.connect_clicked(move |_| {
            pass_entry.set_text(&generate_password(16));
        });
    }
    {
        let dlg = dlg.clone();
        cancel_btn.connect_clicked(move |_| dlg.close());
    }

    {
        let dlg = dlg.clone();
        let entries = entries.clone();
        let master = master.clone();
        let status = status.clone();
        let existing_idx = existing.map(|(i, _)| i);
        save_btn.connect_clicked(move |_| {
            let svc = svc_entry.text().trim().to_string();
            let user = user_entry.text().trim().to_string();
            let pass = pass_entry.text().to_string();
            let notes = notes_entry.text().trim().to_string();
            if svc.is_empty() {
                err.set_text(&tr(lang, "Укажите сервис.", "Service is required."));
                return;
            }
            if pass.is_empty() {
                err.set_text(&tr(lang, "Укажите пароль.", "Password is required."));
                return;
            }
            let entry = Entry {
                service: svc,
                username: user,
                password: pass,
                notes,
            };
            match existing_idx {
                Some(i) => {
                    if let Some(slot) = entries.borrow_mut().get_mut(i) {
                        *slot = entry;
                    } else {
                        err.set_text(&tr(lang, "Запись уже удалена.", "Entry no longer exists."));
                        return;
                    }
                }
                None => entries.borrow_mut().push(entry),
            }
            if persist(lang, &master, &entries, &status) {
                on_done();
                dlg.close();
            }
        });
    }

    dlg.present();
}
