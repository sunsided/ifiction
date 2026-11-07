//! Reads iFiction XML.
//!
//! The reader is tolerant where real records differ from the Treaty: it skips a byte order
//! mark, ignores unknown elements and `<annotation>`, matches elements by local name, and
//! resolves the HTML entities that some records use (`&ldquo;` and friends). It is strict
//! about the values the model types check: IFIDs, dates, numbers and forgiveness.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::Ifid;
use crate::ifid::IfidError;
use crate::model::{
    Auxiliary, Bibliographic, Colophon, Contacts, Cover, CoverFormat, DateError, ForgivenessError,
    Format, FormatSection, Identification, IfIndex, PartialDate, Release, Releases, Story,
};

/// Why a record could not be read.
#[derive(Debug)]
pub enum ReadError {
    Xml(quick_xml::Error),
    Root(String),
    Missing(&'static str, &'static str),
    Number {
        element: String,
        value: String,
    },
    Ifid {
        element: &'static str,
        source: IfidError,
    },
    Date(DateError),
    Forgiveness(ForgivenessError),
    CoverFormat(String),
    Unbalanced,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xml(e) => write!(f, "malformed XML: {e}"),
            Self::Root(name) => write!(f, "the root element is <{name}>, not <ifindex>"),
            Self::Missing(child, parent) => write!(f, "<{child}> is missing in <{parent}>"),
            Self::Number { element, value } => {
                write!(f, "<{element}> has an invalid number: {value:?}")
            }
            Self::Ifid { element, source } => write!(f, "<{element}>: {source}"),
            Self::Date(e) => e.fmt(f),
            Self::Forgiveness(e) => e.fmt(f),
            Self::CoverFormat(v) => write!(f, "cover format is jpg or png, not {v:?}"),
            Self::Unbalanced => f.write_str("unbalanced XML"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Xml(e) => Some(e),
            Self::Ifid { source, .. } => Some(source),
            Self::Date(e) => Some(e),
            Self::Forgiveness(e) => Some(e),
            _ => None,
        }
    }
}

impl From<quick_xml::Error> for ReadError {
    fn from(e: quick_xml::Error) -> Self {
        Self::Xml(e)
    }
}

impl From<DateError> for ReadError {
    fn from(e: DateError) -> Self {
        Self::Date(e)
    }
}

impl From<ForgivenessError> for ReadError {
    fn from(e: ForgivenessError) -> Self {
        Self::Forgiveness(e)
    }
}

/// Reads a whole `.iFiction` file.
pub fn from_str(xml: &str) -> Result<IfIndex, ReadError> {
    let root = parse_tree(xml.trim_start_matches('\u{feff}'))?;
    if root.name != "ifindex" {
        return Err(ReadError::Root(root.name));
    }
    let stories = root
        .elements("story")
        .map(story)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(IfIndex { stories })
}

/// Only the IFIDs of every story, in order. Useful for files that are otherwise invalid.
pub fn ifids(xml: &str) -> Result<Vec<Ifid>, ReadError> {
    let root = parse_tree(xml.trim_start_matches('\u{feff}'))?;
    let mut out = Vec::new();
    for s in root.elements("story") {
        if let Some(id) = s.child("identification") {
            for e in id.elements("ifid") {
                out.push(Ifid::parse(&e.text()).map_err(|source| ReadError::Ifid {
                    element: "ifid",
                    source,
                })?);
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Default)]
struct Element {
    name: String,
    nodes: Vec<Node>,
}

#[derive(Debug)]
enum Node {
    Text(String),
    Element(Element),
}

impl Element {
    fn elements<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.nodes.iter().filter_map(move |n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    fn all_elements(&self) -> impl Iterator<Item = &Element> {
        self.nodes.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    fn child(&self, name: &str) -> Option<&Element> {
        self.nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    /// The text content with whitespace runs collapsed; `<br/>` becomes `\n`.
    fn text(&self) -> String {
        let mut paragraphs = vec![String::new()];
        self.collect_text(&mut paragraphs);
        paragraphs
            .iter()
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn collect_text(&self, paragraphs: &mut Vec<String>) {
        for n in &self.nodes {
            match n {
                Node::Text(t) => paragraphs.last_mut().expect("never empty").push_str(t),
                Node::Element(e) if e.name == "br" => paragraphs.push(String::new()),
                Node::Element(e) => e.collect_text(paragraphs),
            }
        }
    }

    fn opt_text(&self, name: &str) -> Option<String> {
        self.child(name)
            .map(Element::text)
            .filter(|t| !t.is_empty())
    }

    fn req_text(&self, name: &'static str, parent: &'static str) -> Result<String, ReadError> {
        self.opt_text(name).ok_or(ReadError::Missing(name, parent))
    }

    fn opt_number(&self, name: &str) -> Result<Option<u32>, ReadError> {
        self.opt_text(name)
            .map(|v| {
                v.parse().map_err(|_| ReadError::Number {
                    element: name.to_owned(),
                    value: v,
                })
            })
            .transpose()
    }

    fn opt_date(&self, name: &str) -> Result<Option<PartialDate>, ReadError> {
        Ok(self.opt_text(name).map(|v| v.parse()).transpose()?)
    }
}

fn parse_tree(xml: &str) -> Result<Element, ReadError> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Element> = vec![Element::default()];
    loop {
        match reader.read_event()? {
            Event::Start(s) => stack.push(Element {
                name: s.local_name().into_inner().to_owned(),
                nodes: Vec::new(),
            }),
            Event::Empty(s) => {
                let e = Element {
                    name: s.local_name().into_inner().to_owned(),
                    nodes: Vec::new(),
                };
                push_node(&mut stack, Node::Element(e))?;
            }
            Event::End(_) => {
                let e = stack.pop().ok_or(ReadError::Unbalanced)?;
                push_node(&mut stack, Node::Element(e))?;
            }
            Event::Text(t) => push_node(&mut stack, Node::Text(t.xml10_content().into_owned()))?,
            Event::CData(t) => push_node(&mut stack, Node::Text(t.into_inner().into_owned()))?,
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref()? {
                    Some(c) => c.to_string(),
                    None => resolve_entity(&r).map_or_else(|| format!("&{};", &*r), str::to_owned),
                };
                push_node(&mut stack, Node::Text(resolved))?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut doc = stack.pop().ok_or(ReadError::Unbalanced)?;
    if !stack.is_empty() {
        return Err(ReadError::Unbalanced);
    }
    doc.nodes
        .drain(..)
        .find_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
        .ok_or(ReadError::Root(String::new()))
}

fn push_node(stack: &mut [Element], node: Node) -> Result<(), ReadError> {
    stack
        .last_mut()
        .ok_or(ReadError::Unbalanced)?
        .nodes
        .push(node);
    Ok(())
}

/// The XML entities, and the HTML ones seen in real records. Others stay as written.
fn resolve_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "ldquo" => "\u{201c}",
        "rdquo" => "\u{201d}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "hellip" => "\u{2026}",
        "copy" => "\u{a9}",
        "eacute" => "\u{e9}",
        _ => return None,
    })
}

fn ifid_list(e: &Element) -> Result<Vec<Ifid>, ReadError> {
    e.elements("ifid")
        .map(|i| {
            Ifid::parse(&i.text()).map_err(|source| ReadError::Ifid {
                element: "ifid",
                source,
            })
        })
        .collect()
}

fn story(s: &Element) -> Result<Story, ReadError> {
    let id = s
        .child("identification")
        .ok_or(ReadError::Missing("identification", "story"))?;
    let identification = Identification {
        ifids: ifid_list(id)?,
        format: Format::from(id.req_text("format", "identification")?),
        tuid: id.opt_text("tuid"),
        bafn: id.opt_number("bafn")?,
    };
    if identification.ifids.is_empty() {
        return Err(ReadError::Missing("ifid", "identification"));
    }

    let b = s
        .child("bibliographic")
        .ok_or(ReadError::Missing("bibliographic", "story"))?;
    let bibliographic = Bibliographic {
        title: b.req_text("title", "bibliographic")?,
        author: b.req_text("author", "bibliographic")?,
        language: b.opt_text("language"),
        headline: b.opt_text("headline"),
        first_published: b.opt_date("firstpublished")?,
        genre: b.opt_text("genre"),
        group: b.opt_text("group"),
        description: b.opt_text("description"),
        series: b.opt_text("series"),
        series_number: b.opt_number("seriesnumber")?,
        forgiveness: b.opt_text("forgiveness").map(|f| f.parse()).transpose()?,
    };

    let resources = s
        .child("resources")
        .map(|r| {
            r.elements("auxiliary")
                .map(|a| {
                    Ok(Auxiliary {
                        leafname: a.req_text("leafname", "auxiliary")?,
                        description: a.req_text("description", "auxiliary")?,
                    })
                })
                .collect::<Result<Vec<_>, ReadError>>()
        })
        .transpose()?
        .unwrap_or_default();

    let contacts = s.child("contacts").map(|c| Contacts {
        url: c.opt_text("url"),
        author_email: c.opt_text("authoremail"),
    });

    let cover = s.child("cover").map(cover).transpose()?;

    let releases = s
        .child("releases")
        .map(|r| -> Result<Releases, ReadError> {
            Ok(Releases {
                attached: r
                    .child("attached")
                    .and_then(|a| a.child("release"))
                    .map(release)
                    .transpose()?,
                history: r
                    .child("history")
                    .map(|h| h.elements("release").map(release).collect())
                    .transpose()?
                    .unwrap_or_default(),
            })
        })
        .transpose()?;

    let colophon = s
        .child("colophon")
        .map(|c| -> Result<Colophon, ReadError> {
            Ok(Colophon {
                generator: c.req_text("generator", "colophon")?,
                generator_version: c.opt_text("generatorversion"),
                originated: c
                    .opt_date("originated")?
                    .ok_or(ReadError::Missing("originated", "colophon"))?,
            })
        })
        .transpose()?;

    let format_name = identification.format.as_str().to_owned();
    let format_section = s.child(&format_name).map(|f| FormatSection {
        name: format_name.clone(),
        entries: f
            .all_elements()
            .map(|e| (e.name.clone(), e.text()))
            .collect(),
    });

    Ok(Story {
        identification,
        bibliographic,
        resources,
        contacts,
        cover,
        releases,
        colophon,
        format_section,
    })
}

fn cover(c: &Element) -> Result<Cover, ReadError> {
    let format = match c.req_text("format", "cover")?.as_str() {
        "jpg" => CoverFormat::Jpg,
        "png" => CoverFormat::Png,
        other => return Err(ReadError::CoverFormat(other.to_owned())),
    };
    Ok(Cover {
        format,
        width: c
            .opt_number("width")?
            .ok_or(ReadError::Missing("width", "cover"))?,
        height: c
            .opt_number("height")?
            .ok_or(ReadError::Missing("height", "cover"))?,
        description: c.opt_text("description"),
    })
}

fn release(r: &Element) -> Result<Release, ReadError> {
    Ok(Release {
        release_date: r
            .opt_date("releasedate")?
            .ok_or(ReadError::Missing("releasedate", "release"))?,
        version: r.opt_number("version")?,
        compiler: r.opt_text("compiler"),
        compiler_version: r.opt_text("compilerversion"),
    })
}
