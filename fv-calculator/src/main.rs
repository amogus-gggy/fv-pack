//! fv-calculator — простой калькулятор. RU по умолчанию, EN в настройках (⚙).

use gtk::prelude::*;
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
    dir.push("fv-calculator");
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

fn tr_err(lang: Lang, e: &str) -> String {
    if lang == Lang::En {
        return e.to_string();
    }
    if let Some(rest) = e.strip_prefix("unexpected character ") {
        return format!("неожиданный символ {rest}");
    }
    match e {
        "empty expression" => "пустое выражение".into(),
        "bad number" => "плохое число".into(),
        "stray '-'" => "лишний '-'".into(),
        "mismatched parentheses" => "несогласованные скобки".into(),
        "incomplete expression" => "неполное выражение".into(),
        "division by zero" => "деление на ноль".into(),
        "unknown operator" => "неизвестный оператор".into(),
        "bad expression" => "плохое выражение".into(),
        _ => e.to_string(),
    }
}

// ---------- expression evaluator ----------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Op(char), // + - * / % ^
    LParen,
    RParen,
}

fn precedence(op: char) -> u8 {
    match op {
        '+' | '-' => 1,
        '*' | '/' | '%' => 2,
        '^' => 3,
        _ => 0,
    }
}

fn right_assoc(op: char) -> bool {
    op == '^'
}

fn tokenize(expr: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let mut chars = expr.chars().peekable();
    let mut expect_unary = true;

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_digit() || c == '.' || (c == '-' && expect_unary) {
            let mut s = String::new();
            if c == '-' {
                s.push(c);
                chars.next();
                if let Some(&n) = chars.peek() {
                    if n == '(' {
                        toks.push(Tok::Num(-1.0));
                        toks.push(Tok::Op('*'));
                        expect_unary = true;
                        continue;
                    }
                    if !n.is_ascii_digit() && n != '.' {
                        return Err("stray '-'".into());
                    }
                } else {
                    return Err("stray '-'".into());
                }
            }
            let mut dots = 0;
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    s.push(d);
                    chars.next();
                } else if d == '.' {
                    dots += 1;
                    if dots > 1 {
                        return Err("bad number".into());
                    }
                    s.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            if s == "-" || s == "." || s == "-." {
                return Err("bad number".into());
            }
            s.parse::<f64>()
                .map(|n| toks.push(Tok::Num(n)))
                .map_err(|_| "bad number".to_string())?;
            expect_unary = false;
        } else if "+*/%^".contains(c) {
            toks.push(Tok::Op(c));
            chars.next();
            expect_unary = true;
        } else if c == '-' {
            toks.push(Tok::Op(c));
            chars.next();
            expect_unary = true;
        } else if c == '(' {
            toks.push(Tok::LParen);
            chars.next();
            expect_unary = true;
        } else if c == ')' {
            toks.push(Tok::RParen);
            chars.next();
            expect_unary = false;
        } else {
            return Err(format!("unexpected character '{c}'"));
        }
    }
    Ok(toks)
}

fn to_rpn(toks: &[Tok]) -> Result<Vec<Tok>, String> {
    let mut out: Vec<Tok> = Vec::new();
    let mut stack: Vec<Tok> = Vec::new();
    for t in toks {
        match t {
            Tok::Num(_) => out.push(t.clone()),
            Tok::Op(o1) => {
                while let Some(Tok::Op(o2)) = stack.last() {
                    if (precedence(*o2) > precedence(*o1))
                        || (precedence(*o2) == precedence(*o1) && !right_assoc(*o1))
                    {
                        out.push(stack.pop().unwrap());
                    } else {
                        break;
                    }
                }
                stack.push(t.clone());
            }
            Tok::LParen => stack.push(t.clone()),
            Tok::RParen => {
                let mut found = false;
                while let Some(top) = stack.pop() {
                    if top == Tok::LParen {
                        found = true;
                        break;
                    }
                    out.push(top);
                }
                if !found {
                    return Err("mismatched parentheses".into());
                }
            }
        }
    }
    while let Some(top) = stack.pop() {
        match top {
            Tok::LParen | Tok::RParen => return Err("mismatched parentheses".into()),
            _ => out.push(top),
        }
    }
    Ok(out)
}

fn eval_rpn(rpn: &[Tok]) -> Result<f64, String> {
    let mut stack: Vec<f64> = Vec::new();
    for t in rpn {
        match t {
            Tok::Num(n) => stack.push(*n),
            Tok::Op(op) => {
                if stack.len() < 2 {
                    return Err("incomplete expression".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let r = match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => {
                        if b == 0.0 {
                            return Err("division by zero".into());
                        }
                        a / b
                    }
                    '%' => {
                        if b == 0.0 {
                            return Err("division by zero".into());
                        }
                        a % b
                    }
                    '^' => a.powf(b),
                    _ => return Err("unknown operator".into()),
                };
                stack.push(r);
            }
            _ => return Err("bad expression".into()),
        }
    }
    if stack.len() != 1 {
        return Err("incomplete expression".into());
    }
    Ok(stack[0])
}

/// Evaluate an infix expression like "(2+3)*4 - 10/2".
pub fn eval_expr(expr: &str) -> Result<f64, String> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Err("empty expression".into());
    }
    let toks = tokenize(expr)?;
    if toks.is_empty() {
        return Err("empty expression".into());
    }
    let rpn = to_rpn(&toks)?;
    eval_rpn(&rpn)
}

fn format_result(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.10}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ---------- UI ----------

fn main() {
    let app = gtk::Application::new(Some("org.fvpack.calculator"), Default::default());
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
    window.set_default_size(320, 480);
    window.set_resizable(false);

    let header = gtk::HeaderBar::new();
    let header_title = gtk::Label::new(None);
    header.set_title_widget(Some(&header_title));
    window.set_titlebar(Some(&header));
    let settings_btn = gtk::Button::with_label("⚙");
    settings_btn.set_tooltip_text(Some("Настройки / Settings"));
    header.pack_end(&settings_btn);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    window.set_child(Some(&vbox));

    let display = gtk::Entry::new();
    display.set_hexpand(true);
    gtk::prelude::EntryExt::set_alignment(&display, 1.0);
    display.set_placeholder_text(Some("0"));
    display.add_css_class("title-1");
    vbox.append(&display);

    let error_label = gtk::Label::new(None);
    error_label.set_halign(gtk::Align::End);
    error_label.add_css_class("dim-label");
    error_label.add_css_class("error");
    vbox.append(&error_label);

    let grid = gtk::Grid::new();
    grid.set_row_spacing(6);
    grid.set_column_spacing(6);
    grid.set_hexpand(true);
    grid.set_vexpand(true);
    vbox.append(&grid);

    let history_label = gtk::Label::new(None);
    history_label.set_halign(gtk::Align::Start);
    history_label.add_css_class("dim-label");
    vbox.append(&history_label);

    let history_scroll = gtk::ScrolledWindow::new();
    history_scroll.set_min_content_height(90);
    history_scroll.set_vexpand(false);
    vbox.append(&history_scroll);

    let history = gtk::ListBox::new();
    history.set_selection_mode(gtk::SelectionMode::None);
    history_scroll.set_child(Some(&history));

    let history_items: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let apply_lang: Rc<dyn Fn()> = {
        let lang = lang.clone();
        let window = window.clone();
        let header_title = header_title.clone();
        let history_label = history_label.clone();
        Rc::new(move || {
            let lg = *lang.borrow();
            window.set_title(Some(&tr(lg, "Калькулятор", "Calculator")));
            header_title.set_text(&tr(lg, "Калькулятор", "Calculator"));
            history_label.set_text(&tr(lg, "История", "History"));
        })
    };
    apply_lang();

    {
        let window = window.clone();
        let lang = lang.clone();
        let apply_lang = apply_lang.clone();
        settings_btn.connect_clicked(move |_| open_settings(&window, &lang, apply_lang.clone()));
    }

    let do_equals: Rc<dyn Fn()> = {
        let display = display.clone();
        let error_label = error_label.clone();
        let history = history.clone();
        let history_items = history_items.clone();
        let lang = lang.clone();
        Rc::new(move || {
            let expr = display.text().to_string();
            match eval_expr(&expr) {
                Ok(v) => {
                    let out = format_result(v);
                    error_label.set_text("");
                    display.set_text(&out);
                    display.set_position(-1);
                    let line = format!("{expr} = {out}");
                    history_items.borrow_mut().push(line.clone());
                    let row = gtk::ListBoxRow::new();
                    let lbl = gtk::Label::new(Some(&line));
                    lbl.set_halign(gtk::Align::Start);
                    lbl.set_margin_start(6);
                    row.set_child(Some(&lbl));
                    history.prepend(&row);
                    while history_items.borrow().len() > 20 {
                        history_items.borrow_mut().remove(0);
                        if let Some(last) = history.last_child() {
                            history.remove(&last);
                        }
                    }
                }
                Err(e) => {
                    error_label.set_text(&tr_err(*lang.borrow(), &e));
                }
            }
        })
    };

    let buttons: &[(&str, i32, i32, i32)] = &[
        ("C", 0, 0, 1),
        ("(", 1, 0, 1),
        (")", 2, 0, 1),
        ("⌫", 3, 0, 1),
        ("7", 0, 1, 1),
        ("8", 1, 1, 1),
        ("9", 2, 1, 1),
        ("/", 3, 1, 1),
        ("4", 0, 2, 1),
        ("5", 1, 2, 1),
        ("6", 2, 2, 1),
        ("*", 3, 2, 1),
        ("1", 0, 3, 1),
        ("2", 1, 3, 1),
        ("3", 2, 3, 1),
        ("-", 3, 3, 1),
        ("0", 0, 4, 1),
        (".", 1, 4, 1),
        ("=", 2, 4, 1),
        ("+", 3, 4, 1),
        ("%", 0, 5, 2),
        ("^", 2, 5, 2),
    ];

    for (label, col, row, span) in buttons {
        let btn = gtk::Button::with_label(label);
        btn.set_hexpand(true);
        btn.set_vexpand(true);
        if *label == "=" {
            btn.add_css_class("suggested-action");
        }
        let display = display.clone();
        let error_label = error_label.clone();
        let do_equals = do_equals.clone();
        let label = label.to_string();
        btn.connect_clicked(move |_| {
            error_label.set_text("");
            match label.as_str() {
                "C" => display.set_text(""),
                "⌫" => {
                    let t = display.text().to_string();
                    let mut chars: Vec<char> = t.chars().collect();
                    chars.pop();
                    display.set_text(&chars.into_iter().collect::<String>());
                }
                "=" => do_equals(),
                s => {
                    let mut t = display.text().to_string();
                    t.push_str(s);
                    display.set_text(&t);
                    display.set_position(-1);
                }
            }
        });
        grid.attach(&btn, *col, *row, *span, 1);
    }

    {
        let do_equals = do_equals.clone();
        display.connect_activate(move |_| do_equals());
    }

    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_ops() {
        assert_eq!(eval_expr("2+3").unwrap(), 5.0);
        assert_eq!(eval_expr("2+3*4").unwrap(), 14.0);
        assert_eq!(eval_expr("(2+3)*4").unwrap(), 20.0);
        assert_eq!(eval_expr("10/2").unwrap(), 5.0);
        assert_eq!(eval_expr("10%3").unwrap(), 1.0);
        assert_eq!(eval_expr("-5+2").unwrap(), -3.0);
        assert_eq!(eval_expr("2^3").unwrap(), 8.0);
    }

    #[test]
    fn errors() {
        assert!(eval_expr("1/0").is_err());
        assert!(eval_expr("(2+3").is_err());
        assert!(eval_expr("2++3").is_err());
        assert!(eval_expr("").is_err());
    }

    #[test]
    fn ru_errors() {
        assert_eq!(tr_err(Lang::Ru, "division by zero"), "деление на ноль");
        assert_eq!(tr_err(Lang::Ru, "empty expression"), "пустое выражение");
        assert_eq!(tr_err(Lang::En, "division by zero"), "division by zero");
    }
}
