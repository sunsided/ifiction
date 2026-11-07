//! Writes iFiction XML.
//!
//! The output is deterministic: sections and fields always come in the same order, with
//! two-space indentation. Text uses exactly the three escapes the Treaty allows:
//! `&amp;`, `&lt;` and `&gt;` (section 5.6).

use std::fmt::Write as _;

use crate::model::{
    Auxiliary, Bibliographic, Colophon, Contacts, Cover, Forgiveness, FormatSection,
    Identification, IfIndex, NAMESPACE, Release, Releases, Story,
};

/// The XML declaration that starts every record.
pub const XML_DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

/// Escapes `&`, `<` and `>`, and nothing else.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// Turns description paragraphs (separated by `\n`) into escaped text with `<br/>`.
pub fn description_markup(text: &str) -> String {
    text.split('\n')
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .map(|p| escape(&p))
        .collect::<Vec<_>>()
        .join("<br/>")
}

/// Writes a whole `.iFiction` file.
pub fn to_string(index: &IfIndex) -> String {
    let mut w = Writer::default();
    w.out.push_str(XML_DECLARATION);
    w.out.push('\n');
    let _ = writeln!(w.out, r#"<ifindex version="1.0" xmlns="{NAMESPACE}">"#);
    w.depth = 1;
    for story in &index.stories {
        w.story(story);
    }
    w.out.push_str("</ifindex>\n");
    w.out
}

#[derive(Default)]
struct Writer {
    out: String,
    depth: usize,
}

impl Writer {
    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    fn open(&mut self, tag: &str) {
        self.indent();
        let _ = writeln!(self.out, "<{tag}>");
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.indent();
        let _ = writeln!(self.out, "</{tag}>");
    }

    /// An element with escaped text.
    fn text(&mut self, tag: &str, value: &str) {
        self.raw(tag, &escape(value));
    }

    /// An element whose content is already markup.
    fn raw(&mut self, tag: &str, markup: &str) {
        self.indent();
        let _ = writeln!(self.out, "<{tag}>{markup}</{tag}>");
    }

    fn opt(&mut self, tag: &str, value: Option<&str>) {
        if let Some(v) = value {
            self.text(tag, v);
        }
    }

    fn opt_display(&mut self, tag: &str, value: Option<impl ToString>) {
        if let Some(v) = value {
            self.text(tag, &v.to_string());
        }
    }

    fn story(&mut self, s: &Story) {
        self.open("story");
        self.identification(&s.identification);
        self.bibliographic(&s.bibliographic);
        if !s.resources.is_empty() {
            self.resources(&s.resources);
        }
        if let Some(c) = &s.contacts {
            self.contacts(c);
        }
        if let Some(c) = &s.cover {
            self.cover(c);
        }
        if let Some(r) = &s.releases {
            self.releases(r);
        }
        if let Some(c) = &s.colophon {
            self.colophon(c);
        }
        if let Some(f) = &s.format_section {
            self.format_section(f);
        }
        self.close("story");
    }

    fn identification(&mut self, id: &Identification) {
        self.open("identification");
        for ifid in &id.ifids {
            self.text("ifid", ifid.as_str());
        }
        self.text("format", id.format.as_str());
        self.opt("tuid", id.tuid.as_deref());
        self.opt_display("bafn", id.bafn);
        self.close("identification");
    }

    fn bibliographic(&mut self, b: &Bibliographic) {
        self.open("bibliographic");
        self.text("title", &b.title);
        self.text("author", &b.author);
        self.opt("language", b.language.as_deref());
        self.opt("headline", b.headline.as_deref());
        self.opt_display("firstpublished", b.first_published);
        self.opt("genre", b.genre.as_deref());
        self.opt("group", b.group.as_deref());
        if let Some(d) = &b.description {
            self.raw("description", &description_markup(d));
        }
        self.opt("series", b.series.as_deref());
        self.opt_display("seriesnumber", b.series_number);
        self.opt("forgiveness", b.forgiveness.map(Forgiveness::as_str));
        self.close("bibliographic");
    }

    fn resources(&mut self, resources: &[Auxiliary]) {
        self.open("resources");
        for a in resources {
            self.open("auxiliary");
            self.text("leafname", &a.leafname);
            self.text("description", &a.description);
            self.close("auxiliary");
        }
        self.close("resources");
    }

    fn contacts(&mut self, c: &Contacts) {
        self.open("contacts");
        self.opt("url", c.url.as_deref());
        self.opt("authoremail", c.author_email.as_deref());
        self.close("contacts");
    }

    fn cover(&mut self, c: &Cover) {
        self.open("cover");
        self.text("format", c.format.as_str());
        self.text("height", &c.height.to_string());
        self.text("width", &c.width.to_string());
        self.opt("description", c.description.as_deref());
        self.close("cover");
    }

    fn releases(&mut self, r: &Releases) {
        self.open("releases");
        if let Some(a) = &r.attached {
            self.open("attached");
            self.release(a);
            self.close("attached");
        }
        if !r.history.is_empty() {
            self.open("history");
            for h in &r.history {
                self.release(h);
            }
            self.close("history");
        }
        self.close("releases");
    }

    fn release(&mut self, r: &Release) {
        self.open("release");
        self.opt_display("version", r.version);
        self.text("releasedate", &r.release_date.to_string());
        self.opt("compiler", r.compiler.as_deref());
        self.opt("compilerversion", r.compiler_version.as_deref());
        self.close("release");
    }

    fn colophon(&mut self, c: &Colophon) {
        self.open("colophon");
        self.text("generator", &c.generator);
        self.opt("generatorversion", c.generator_version.as_deref());
        self.text("originated", &c.originated.to_string());
        self.close("colophon");
    }

    fn format_section(&mut self, f: &FormatSection) {
        self.open(&f.name);
        for (k, v) in &f.entries {
            self.text(k, v);
        }
        self.close(&f.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_exactly_three() {
        assert_eq!(
            escape(r#"A & B <C> "D" 'E'"#),
            r#"A &amp; B &lt;C&gt; "D" 'E'"#
        );
    }

    #[test]
    fn description_paragraphs() {
        assert_eq!(
            description_markup("  One  line\n\n Two & \tthree \n"),
            "One line<br/>Two &amp; three"
        );
    }
}
