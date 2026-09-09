//! fv-calendar — простой календарь с заметками и подсветкой в сетке.
//! RU by default, EN via settings (⚙).

use chrono::{Datelike, Local, NaiveDate, Weekday};
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

type Events = HashMap<String, Vec<String>>;

// ---------- i18n + settings ----------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ru,
    En,
}

fn settings_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("fv-calendar");
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
    Lang::Ru // default
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

// ---------- data ----------

fn data_file() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("fv-calendar");
    let _ = fs::create_dir_all(&dir);
    dir.push("events.json");
    dir
}

fn load_events() -> Events {
    let path = data_file();
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(map) = serde_json::from_slice::<Events>(&bytes) {
            return map;
        }
    }
    HashMap::new()
}

fn save_events(events: &Events) {
    let path = data_file();
    if let Ok(json) = serde_json::to_string_pretty(events) {
        let _ = fs::write(path, json);
    }
}

fn date_key(year: i32, month: u32, day: u32) -> String {
    format!("{:04}-{month:02}-{day:02}", year)
}

const RU_WEEKDAYS_FULL: [&str; 7] = [
    "понедельник",
    "вторник",
    "среда",
    "четверг",
    "пятница",
    "суббота",
    "воскресенье",
];
const RU_MONTHS_GEN: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];
const RU_MONTHS_NOM: [&str; 12] = [
    "Январь",
    "Февраль",
    "Март",
    "Апрель",
    "Май",
    "Июнь",
    "Июль",
    "Август",
    "Сентябрь",
    "Октябрь",
    "Ноябрь",
    "Декабрь",
];
const EN_WEEKDAYS_FULL: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const EN_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn pretty_date(lang: Lang, year: i32, month: u32, day: u32) -> String {
    if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
        let wi = d.weekday().number_from_monday() as usize - 1;
        if lang == Lang::Ru {
            format!(
                "{}, {:02} {} {}",
                RU_WEEKDAYS_FULL[wi],
                day,
                RU_MONTHS_GEN[(month - 1) as usize],
                year
            )
        } else {
            format!(
                "{}, {:02} {} {}",
                EN_WEEKDAYS_FULL[wi],
                day,
                EN_MONTHS[(month - 1) as usize],
                year
            )
        }
    } else {
        format!("{year}-{month:02}-{day:02}")
    }
}

fn month_title(lang: Lang, year: i32, month: u32) -> String {
    if !(1..=12).contains(&month) {
        return format!("{year}-{month:02}");
    }
    if lang == Lang::Ru {
        format!("{} {}", RU_MONTHS_NOM[(month - 1) as usize], year)
    } else {
        format!("{} {}", EN_MONTHS[(month - 1) as usize], year)
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    for d in (28..=31u32).rev() {
        if NaiveDate::from_ymd_opt(year, month, d).is_some() {
            return d;
        }
    }
    30
}

fn holiday_name(lang: Lang, month: u32, day: u32) -> Option<&'static str> {
    if lang == Lang::Ru {
        match (month, day) {
            (1, 1) => Some("Новый год"),
            (1, 2) | (1, 3) | (1, 4) | (1, 5) | (1, 6) | (1, 8) => Some("Новогодние каникулы"),
            (1, 7) => Some("Рождество Христово"),
            (2, 23) => Some("День защитника Отечества"),
            (3, 8) => Some("Международный женский день"),
            (5, 1) => Some("День весны и труда"),
            (5, 9) => Some("День Победы"),
            (6, 12) => Some("День России"),
            (11, 4) => Some("День народного единства"),
            (12, 25) => Some("Рождество (западное)"),
            (12, 31) => Some("Канун Нового года"),
            _ => None,
        }
    } else {
        match (month, day) {
            (1, 1) => Some("New Year"),
            (1, 2) | (1, 3) | (1, 4) | (1, 5) | (1, 6) | (1, 8) => Some("New Year holiday"),
            (1, 7) => Some("Orthodox Christmas"),
            (2, 23) => Some("Defender of the Fatherland Day"),
            (3, 8) => Some("International Women's Day"),
            (5, 1) => Some("Spring and Labour Day"),
            (5, 9) => Some("Victory Day"),
            (6, 12) => Some("Russia Day"),
            (11, 4) => Some("Unity Day"),
            (12, 25) => Some("Christmas (Western)"),
            (12, 31) => Some("New Year's Eve"),
            _ => None,
        }
    }
}

fn has_event(events: &Events, year: i32, month: u32, day: u32) -> bool {
    events
        .get(&date_key(year, month, day))
        .is_some_and(|v| !v.is_empty())
}

fn main() {
    let app = gtk::Application::new(Some("org.fvpack.calendar"), Default::default());
    app.connect_activate(build_ui);
    app.run();
}

const GRID_CSS: &str = "
.cal-day { border-radius: 8px; padding: 6px 2px; font-weight: normal; }
.cal-day.cal-today { border: 2px solid #3584e4; }
.cal-holiday { background: rgba(224, 27, 36, 0.22); color: #e01b24; font-weight: bold; }
.cal-event { background: rgba(53, 132, 228, 0.25); font-weight: bold; }
.cal-holiday.cal-event { background: rgba(145, 65, 172, 0.32); color: #984a9c; font-weight: bold; }
.cal-selected { background: #3584e4; color: white; font-weight: bold; }
.cal-selected.cal-holiday { background: #c01c28; color: white; }
.cal-selected.cal-event { background: #3584e4; color: white; }
.cal-selected.cal-holiday.cal-event { background: #813d9c; color: white; }
.cal-other-month { opacity: 0.45; }
.cal-weekend { color: #c01c28; }
.cal-selected.cal-weekend { color: white; }
.cal-dow { font-weight: bold; opacity: 0.7; }
.cal-dow-weekend { color: #c01c28; }
.holiday-label { color: #c01c28; font-weight: bold; }
";

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
        // keep dialog labels in sync is overkill; just save + refresh main UI
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

#[allow(clippy::too_many_lines)]
fn build_ui(app: &gtk::Application) {
    let today = Local::now().date_naive();
    let visible: Rc<RefCell<(i32, u32)>> = Rc::new(RefCell::new((today.year(), today.month())));
    let selected: Rc<RefCell<(i32, u32, u32)>> =
        Rc::new(RefCell::new((today.year(), today.month(), today.day())));
    let events: Rc<RefCell<Events>> = Rc::new(RefCell::new(load_events()));
    let lang: Rc<RefCell<Lang>> = Rc::new(RefCell::new(load_lang()));

    let window = gtk::ApplicationWindow::new(app);
    window.set_default_size(760, 480);

    let css = gtk::CssProvider::new();
    css.load_from_string(GRID_CSS);
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let header = gtk::HeaderBar::new();
    let header_title = gtk::Label::new(None);
    header.set_title_widget(Some(&header_title));
    window.set_titlebar(Some(&header));
    let today_btn = gtk::Button::with_label("");
    header.pack_start(&today_btn);
    let settings_btn = gtk::Button::with_label("⚙");
    settings_btn.set_tooltip_text(Some("Настройки / Settings"));
    header.pack_end(&settings_btn);

    let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    main_box.set_margin_top(12);
    main_box.set_margin_bottom(12);
    main_box.set_margin_start(12);
    main_box.set_margin_end(12);
    window.set_child(Some(&main_box));

    let left = gtk::Box::new(gtk::Orientation::Vertical, 8);
    main_box.append(&left);

    let nav = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    nav.set_halign(gtk::Align::Center);
    left.append(&nav);

    let prev_year = gtk::Button::with_label("«");
    let prev_month = gtk::Button::with_label("‹");
    let month_label = gtk::Label::new(None);
    month_label.add_css_class("title-2");
    month_label.set_hexpand(true);
    month_label.set_halign(gtk::Align::Center);
    month_label.set_width_chars(16);
    let next_month = gtk::Button::with_label("›");
    let next_year = gtk::Button::with_label("»");
    nav.append(&prev_year);
    nav.append(&prev_month);
    nav.append(&month_label);
    nav.append(&next_month);
    nav.append(&next_year);

    let dow_grid = gtk::Grid::new();
    dow_grid.set_column_spacing(4);
    dow_grid.set_column_homogeneous(true);
    left.append(&dow_grid);
    let dow_labels: Vec<gtk::Label> = (0..7).map(|_| gtk::Label::new(None)).collect();
    for (i, l) in dow_labels.iter().enumerate() {
        l.add_css_class("cal-dow");
        if i >= 5 {
            l.add_css_class("cal-dow-weekend");
        }
        l.set_hexpand(true);
        dow_grid.attach(l, i as i32, 0, 1, 1);
    }

    let days_grid = gtk::Grid::new();
    days_grid.set_row_spacing(4);
    days_grid.set_column_spacing(4);
    days_grid.set_column_homogeneous(true);
    days_grid.set_row_homogeneous(true);
    days_grid.set_hexpand(true);
    days_grid.set_vexpand(true);
    left.append(&days_grid);

    let legend = gtk::Label::new(None);
    legend.set_halign(gtk::Align::Center);
    legend.add_css_class("dim-label");
    left.append(&legend);

    let sep = gtk::Separator::new(gtk::Orientation::Vertical);
    main_box.append(&sep);

    let right = gtk::Box::new(gtk::Orientation::Vertical, 8);
    right.set_hexpand(true);
    main_box.append(&right);

    let date_label = gtk::Label::new(None);
    date_label.set_halign(gtk::Align::Start);
    date_label.add_css_class("title-2");
    right.append(&date_label);

    let holiday_label = gtk::Label::new(None);
    holiday_label.set_halign(gtk::Align::Start);
    holiday_label.add_css_class("holiday-label");
    right.append(&holiday_label);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_min_content_height(180);
    right.append(&scrolled);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    scrolled.set_child(Some(&list));

    let entry_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    right.append(&entry_row);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry_row.append(&entry);
    let add_btn = gtk::Button::with_label("");
    add_btn.add_css_class("suggested-action");
    entry_row.append(&add_btn);

    let remove_btn = gtk::Button::with_label("");
    right.append(&remove_btn);

    // ---- single refresh ----
    let refresh_cell: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let refresh: Rc<dyn Fn()> = {
        let visible = visible.clone();
        let selected = selected.clone();
        let events = events.clone();
        let lang = lang.clone();
        let window = window.clone();
        let header_title = header_title.clone();
        let today_btn = today_btn.clone();
        let month_label = month_label.clone();
        let dow_labels = dow_labels.clone();
        let legend = legend.clone();
        let days_grid = days_grid.clone();
        let date_label = date_label.clone();
        let holiday_label = holiday_label.clone();
        let list = list.clone();
        let entry = entry.clone();
        let add_btn = add_btn.clone();
        let remove_btn = remove_btn.clone();
        let refresh_cell = refresh_cell.clone();
        Rc::new(move || {
            let lg = *lang.borrow();
            window.set_title(Some(&tr(lg, "Календарь", "Calendar")));
            header_title.set_text(&tr(lg, "Календарь", "Calendar"));
            today_btn.set_label(&tr(lg, "Сегодня", "Today"));

            let dows_ru = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
            let dows_en = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
            for (i, l) in dow_labels.iter().enumerate() {
                l.set_text(if lg == Lang::Ru {
                    dows_ru[i]
                } else {
                    dows_en[i]
                });
            }
            legend.set_text(&tr(
                lg,
                "■ праздник   ■ заметки   ■ оба",
                "■ holiday   ■ notes   ■ both",
            ));
            entry.set_placeholder_text(Some(&tr(
                lg,
                "Добавить заметку на этот день…",
                "Add a note for this day…",
            )));
            add_btn.set_label(&tr(lg, "Добавить", "Add"));
            remove_btn.set_label(&tr(lg, "Удалить выбранное", "Remove selected"));

            let (vy, vm) = *visible.borrow();
            let (sy, sm, sd) = *selected.borrow();
            let ev = events.borrow();

            month_label.set_text(&month_title(lg, vy, vm));

            while let Some(ch) = days_grid.first_child() {
                days_grid.remove(&ch);
            }

            let first = NaiveDate::from_ymd_opt(vy, vm, 1).expect("valid month");
            let offset = first.weekday().number_from_monday() as i32 - 1;
            let dim = days_in_month(vy, vm) as i32;
            let (py, pm) = if vm == 1 { (vy - 1, 12) } else { (vy, vm - 1) };
            let prev_dim = days_in_month(py, pm) as i32;
            let (ny, nm) = if vm == 12 { (vy + 1, 1) } else { (vy, vm + 1) };

            let t = Local::now().date_naive();
            let (ty, tm, td) = (t.year(), t.month(), t.day());

            for idx in 0..42i32 {
                let raw = idx - offset + 1;
                let (cy, cm, cd, other) = if raw < 1 {
                    (py, pm, (prev_dim + raw) as u32, true)
                } else if raw > dim {
                    (ny, nm, (raw - dim) as u32, true)
                } else {
                    (vy, vm, raw as u32, false)
                };

                let is_today = cy == ty && cm == tm && cd == td;
                let is_sel = cy == sy && cm == sm && cd == sd;
                let is_hol = holiday_name(lg, cm, cd).is_some();
                let is_ev = has_event(&ev, cy, cm, cd);

                let label = if is_ev {
                    format!("{cd} •")
                } else {
                    format!("{cd}")
                };
                let btn = gtk::Button::with_label(&label);
                btn.set_hexpand(true);
                btn.set_vexpand(true);
                btn.add_css_class("cal-day");
                if other {
                    btn.add_css_class("cal-other-month");
                }
                if is_today {
                    btn.add_css_class("cal-today");
                }
                if is_sel {
                    btn.add_css_class("cal-selected");
                }
                if is_hol {
                    btn.add_css_class("cal-holiday");
                }
                if is_ev {
                    btn.add_css_class("cal-event");
                }
                if let Some(d) = NaiveDate::from_ymd_opt(cy, cm, cd) {
                    if matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
                        btn.add_css_class("cal-weekend");
                    }
                }
                if is_hol {
                    if let Some(name) = holiday_name(lg, cm, cd) {
                        btn.set_tooltip_text(Some(name));
                    }
                }

                {
                    let visible = visible.clone();
                    let selected = selected.clone();
                    let refresh_cell = refresh_cell.clone();
                    btn.connect_clicked(move |_| {
                        *selected.borrow_mut() = (cy, cm, cd);
                        if other {
                            *visible.borrow_mut() = (cy, cm);
                        }
                        if let Some(r) = refresh_cell.borrow().clone() {
                            r();
                        }
                    });
                }
                days_grid.attach(&btn, idx % 7, idx / 7, 1, 1);
            }
            drop(ev);

            date_label.set_text(&pretty_date(lg, sy, sm, sd));
            if let Some(name) = holiday_name(lg, sm, sd) {
                holiday_label.set_text(&format!("🎉 {name}"));
                holiday_label.set_visible(true);
            } else {
                holiday_label.set_text("");
                holiday_label.set_visible(false);
            }
            while let Some(row) = list.first_child() {
                list.remove(&row);
            }
            let key = date_key(sy, sm, sd);
            let ev = events.borrow();
            if let Some(items) = ev.get(&key) {
                for item in items {
                    let row = gtk::ListBoxRow::new();
                    let lbl = gtk::Label::new(Some(item));
                    lbl.set_halign(gtk::Align::Start);
                    lbl.set_margin_start(8);
                    lbl.set_margin_end(8);
                    lbl.set_margin_top(4);
                    lbl.set_margin_bottom(4);
                    lbl.set_wrap(true);
                    row.set_child(Some(&lbl));
                    list.append(&row);
                }
            }
            if list.first_child().is_none() {
                let row = gtk::ListBoxRow::new();
                row.set_selectable(false);
                row.set_sensitive(false);
                let lbl = gtk::Label::new(Some(&tr(
                    lg,
                    "Нет заметок на этот день.",
                    "No notes for this day.",
                )));
                lbl.add_css_class("dim-label");
                row.set_child(Some(&lbl));
                list.append(&row);
            }
        })
    };
    *refresh_cell.borrow_mut() = Some(refresh.clone());

    {
        let window = window.clone();
        let lang = lang.clone();
        let refresh = refresh.clone();
        settings_btn.connect_clicked(move |_| open_settings(&window, &lang, refresh.clone()));
    }
    {
        let visible = visible.clone();
        let refresh = refresh.clone();
        prev_month.connect_clicked(move |_| {
            let (y, m) = *visible.borrow();
            *visible.borrow_mut() = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
            refresh();
        });
    }
    {
        let visible = visible.clone();
        let refresh = refresh.clone();
        next_month.connect_clicked(move |_| {
            let (y, m) = *visible.borrow();
            *visible.borrow_mut() = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            refresh();
        });
    }
    {
        let visible = visible.clone();
        let refresh = refresh.clone();
        prev_year.connect_clicked(move |_| {
            visible.borrow_mut().0 -= 1;
            refresh();
        });
    }
    {
        let visible = visible.clone();
        let refresh = refresh.clone();
        next_year.connect_clicked(move |_| {
            visible.borrow_mut().0 += 1;
            refresh();
        });
    }
    {
        let visible = visible.clone();
        let selected = selected.clone();
        let refresh = refresh.clone();
        today_btn.connect_clicked(move |_| {
            let t = Local::now().date_naive();
            *visible.borrow_mut() = (t.year(), t.month());
            *selected.borrow_mut() = (t.year(), t.month(), t.day());
            refresh();
        });
    }
    {
        let selected = selected.clone();
        let events = events.clone();
        let entry_inner = entry.clone();
        let entry_activate = entry.clone();
        let refresh = refresh.clone();
        let do_add = Rc::new(move || {
            let text = entry_inner.text().trim().to_string();
            if text.is_empty() {
                return;
            }
            let (y, m, d) = *selected.borrow();
            events
                .borrow_mut()
                .entry(date_key(y, m, d))
                .or_default()
                .push(text);
            save_events(&events.borrow());
            entry_inner.set_text("");
            refresh();
        });
        let do_add_btn = do_add.clone();
        add_btn.connect_clicked(move |_| do_add_btn());
        entry_activate.connect_activate(move |_| do_add());
    }
    {
        let selected = selected.clone();
        let events = events.clone();
        let refresh = refresh.clone();
        remove_btn.connect_clicked(move |_| {
            let Some(row) = list.selected_row() else {
                return;
            };
            let idx = row.index() as usize;
            let (y, m, d) = *selected.borrow();
            let key = date_key(y, m, d);
            let mut ev = events.borrow_mut();
            if let Some(items) = ev.get_mut(&key) {
                if idx < items.len() {
                    items.remove(idx);
                    if items.is_empty() {
                        ev.remove(&key);
                    }
                    save_events(&ev);
                }
            }
            drop(ev);
            refresh();
        });
    }

    refresh();
    window.present();
}
