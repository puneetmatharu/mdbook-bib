use crate::config::DEFAULT_CSS_TEMPLATE;
use crate::config::DEFAULT_HB_TEMPLATE;
use crate::config::DEFAULT_JS_TEMPLATE;
use crate::Bibiography;
use mdbook::MDBook;
use std::fs::File;
use std::io::Write;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[cfg(test)]
// use std::{println as info, println as warn};
use tempfile::Builder as TempFileBuilder;

use crate::PlaceholderType::{AtCite, Cite};
use crate::{
    build_bibliography, extract_date, find_at_placeholders, find_placeholders, load_bibliography,
    replace_all_placeholders, BibItem, Config,
};
use toml::value::Table;
use toml::Value;

use mdbook::book::Chapter;

static EXAMPLE_CSS_TEMPLATE: &str = include_str!("../manual/src/render/my_style.css");
static EXAMPLE_HB_TEMPLATE: &str = include_str!("../manual/src/render/my_references.hbs");

const DUMMY_BIB_SRC: &str = r#"
@misc {fps,
    author = {"Francisco Perez-Sorrosal"},
    title = {"This is a bib entry!"},
    month = {"oct"},
    year = {"2020"},
    what_is_this = {"blabla"},
}
@book{rust_book,
    author = {"Klabnik, Steve and Nichols, Carol"},
    title = {"The Rust Programming Language"},
    year = {"2018"},
    isbn = {"1593278284"},
    publisher = {"No Starch Press"},
    url = {https://doc.rust-lang.org/book/},
}
@book{book_of_spells,
    author = {"James, Andrew and Wilson, Andrew Lincoln and Francisco Perez-Sorrosal"},
    title = {"The Rust Programming Language"},
    year = {"2019"},
    isbn = {"1593278284"},
    url = {https://doc.rust-lang.org/book/},
    publisher = {"Cambridge University Press"},
    journal = {"Journal of Fluid Mechanics"},
    address = {"Cambridge"},
    volume = {"918"},
}
"#;

const DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS: &str = r#"
this is a dumb text that includes citations like {{ #cite fps }} and {{ #cite rust_book }}
"#;

const DUMMY_TEXT_WITH_A_VALID_AND_AN_INVALID_CITE_PLACEHOLDERS: &str = r#"
this is a dumb text that includes valid and invalid citations like {{ #cite fps }} and {{ #cite im_not_there }}
"#;

const DUMMY_TEXT_WITH_A_VALID_AT_CITE_PLACEHOLDER: &str = r#"
this is a dumb text that includes a valid citation with double @, as in @@fps.
"#;

const DUMMY_TEXT_WITH_2_UNKNOWN_PLACEHOLDERS: &str = r#"
this is a dumb text that includes invalid placeholders like {{ #zoto uhmmmm }} and {{ #peto ahhhhmmm }}
"#;

#[test]
fn load_bib_bibliography_from_file() {
    let temp = TempFileBuilder::new().prefix("book").tempdir().unwrap();
    let chapter_path = temp.path().join("biblio.bib");
    File::create(&chapter_path)
        .unwrap()
        .write_all(DUMMY_BIB_SRC.as_bytes())
        .unwrap();

    let bibliography_loaded: String = load_bibliography(chapter_path.as_path()).unwrap();
    assert_ne!(bibliography_loaded, "");
    assert!(bibliography_loaded.contains("\"Francisco Perez-Sorrosal\""));
}

#[test]
fn cant_load_bib_bibliography_from_file() {
    let temp = TempFileBuilder::new().prefix("book").tempdir().unwrap();
    let chapter_path = temp.path().join("biblio.wrong_extension");
    File::create(&chapter_path)
        .unwrap()
        .write_all(DUMMY_BIB_SRC.as_bytes())
        .unwrap();

    let bibliography_loaded: String = load_bibliography(chapter_path.as_path()).unwrap();
    assert_eq!(bibliography_loaded, "");
}

#[test]
fn bibliography_builder_returns_a_bibliography() {
    let bibliography_loaded: HashMap<String, BibItem> =
        build_bibliography(DUMMY_BIB_SRC.to_string()).unwrap();
    assert_eq!(bibliography_loaded.len(), 3);
    assert_eq!(bibliography_loaded.get("fps").unwrap().citation_key, "fps");
}

#[test]
fn bibliography_render_all_vs_cited() {
    let bibliography_loaded: HashMap<String, BibItem> =
        build_bibliography(DUMMY_BIB_SRC.to_string()).unwrap();

    let mut cited = HashSet::new();
    cited.insert("fps".to_string());

    let html = Bibiography::generate_bibliography_html(
        &bibliography_loaded,
        &cited,
        false,
        format!("\n\n{}\n\n", DEFAULT_HB_TEMPLATE),
    );

    assert!(html.contains("This is a bib entry!"));
    assert!(html.contains("The Rust Programming Language"));

    let html = Bibiography::generate_bibliography_html(
        &bibliography_loaded,
        &cited,
        true,
        format!("\n\n{}\n\n", DEFAULT_HB_TEMPLATE),
    );

    assert!(html.contains("This is a bib entry!"));
    assert!(!html.contains("The Rust Programming Language"));
}

#[test]
fn bibliography_includes_and_renders_url_when_present_in_bibitems() {
    let bibliography_loaded: HashMap<String, BibItem> =
        build_bibliography(DUMMY_BIB_SRC.to_string()).unwrap();

    // book_of_spells contains a publisher, journal, address and volume
    let book_of_spells = bibliography_loaded.get("book_of_spells");
    assert_eq!(&book_of_spells.unwrap().authors, "James et al.");
    assert_eq!(
        book_of_spells.unwrap().publisher.as_ref().unwrap(),
        "Cambridge University Press"
    );
    assert_eq!(
        book_of_spells.unwrap().journal.as_ref().unwrap(),
        "Journal of Fluid Mechanics"
    );
    assert_eq!(
        book_of_spells.unwrap().address.as_ref().unwrap(),
        "Cambridge"
    );
    assert_eq!(book_of_spells.unwrap().volume.as_ref().unwrap(), "918");

    // fps dummy book does not include a url for in the BibItem
    let fps = bibliography_loaded.get("fps");
    assert_eq!(&fps.unwrap().authors, "Francisco",);
    assert!(fps.unwrap().url.is_none());
    assert!(fps.unwrap().journal.is_none());

    // rust_book does...
    let rust_book = bibliography_loaded.get("rust_book");
    assert_eq!(&rust_book.unwrap().authors, "Klabnik & Nichols",);
    assert_eq!(
        rust_book.unwrap().url.as_ref().unwrap(),
        "https://doc.rust-lang.org/book/"
    );
    assert!(rust_book.unwrap().address.is_none());

    // ...and is included in the render
    let html = Bibiography::generate_bibliography_html(
        &bibliography_loaded,
        &HashSet::new(),
        false,
        format!("\n\n{}\n\n", DEFAULT_HB_TEMPLATE),
    );
    assert!(html.contains("href=\"https://doc.rust-lang.org/book/\""));
}

#[test]
fn valid_and_invalid_citations_are_replaced_properly_in_book_text() {
    let bibliography: HashMap<String, BibItem> =
        build_bibliography(DUMMY_BIB_SRC.to_string()).unwrap();

    let mut cited: HashSet<String> = HashSet::new();

    // Check valid references included in a dummy text
    let chapter = Chapter::new(
        "",
        DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS.into(),
        "source.md",
        vec![],
    );
    let text_with_citations = replace_all_placeholders(&chapter, &bibliography, &mut cited);
    // TODO: These asserts will probably fail if we allow users to specify the bibliography
    // chapter name as per issue #6
    assert!(text_with_citations.contains("[Francisco, 2020](bibliography.html#fps)"));
    assert!(text_with_citations.contains("[Klabnik & Nichols, 2018](bibliography.html#rust_book)"));

    // Check a mix of valid and invalid references included/not included in a dummy text
    let chapter = Chapter::new(
        "",
        DUMMY_TEXT_WITH_A_VALID_AND_AN_INVALID_CITE_PLACEHOLDERS.into(),
        "source.md",
        vec![],
    );
    let text_with_citations = replace_all_placeholders(&chapter, &bibliography, &mut cited);
    assert!(text_with_citations.contains("[Francisco, 2020]"));
    assert!(text_with_citations.contains("[Unknown bib ref:"));
}

#[test]
fn citations_in_subfolders_link_properly() {
    let bibliography: HashMap<String, BibItem> =
        build_bibliography(DUMMY_BIB_SRC.to_string()).unwrap();

    // Check valid references included in a dummy text
    let check_citations_for = |chapter: &Chapter, link: &str| {
        let text_with_citations =
            replace_all_placeholders(chapter, &bibliography, &mut HashSet::new());

        // TODO: These asserts will probably fail if we allow users to specify the bibliography
        // chapter name as per issue #6
        assert!(
            text_with_citations.contains(&format!("[Francisco, 2020]({}#fps)", link)),
            "Expecting link to '{}' in string '{}'",
            link,
            text_with_citations
        );
        assert!(
            text_with_citations.contains(&format!("[Klabnik & Nichols, 2018]({}#rust_book)", link)),
            "Expecting link to '{}' in string '{}'",
            link,
            text_with_citations
        );
    };

    let mut draft_chapter = Chapter::new_draft("", vec![]);
    draft_chapter.content = DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS.into();
    check_citations_for(&draft_chapter, "bibliography.html");

    let chapter_root = Chapter::new(
        "",
        DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS.into(),
        "source.md",
        vec![],
    );
    check_citations_for(&chapter_root, "bibliography.html");

    let chapter_1down = Chapter::new(
        "",
        DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS.into(),
        "dir1/source.md",
        vec![],
    );
    check_citations_for(&chapter_1down, "../bibliography.html");

    let chapter_2down = Chapter::new(
        "",
        DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS.into(),
        "dir1/dir2/source.md",
        vec![],
    );
    check_citations_for(&chapter_2down, "../../bibliography.html");

    let chapter_noncanon = Chapter::new(
        "",
        DUMMY_TEXT_WITH_2_VALID_CITE_PLACEHOLDERS.into(),
        "dir1/dir2/../source.md",
        vec![],
    );
    check_citations_for(&chapter_noncanon, "../bibliography.html");
}

#[test]
fn find_only_citation_placeholders() {
    // As long as placeholders are related to cites, they are found, independently of whether they
    // are valid or not
    let plhs = find_placeholders(DUMMY_TEXT_WITH_A_VALID_AND_AN_INVALID_CITE_PLACEHOLDERS);
    let mut items = 0;
    for plh in plhs {
        match plh.placeholder_type {
            Cite(_) => items += 1,
            AtCite(_) => items += 1,
        };
    }
    assert_eq!(items, 2);

    // When no recognized placeholders are found, they are ignored
    let plhs = find_placeholders(DUMMY_TEXT_WITH_2_UNKNOWN_PLACEHOLDERS);
    items = 0;
    for _ in plhs {
        panic!("Only Cite should be recognized as placeholder type!!!");
    }
    assert_eq!(items, 0);
}

#[test]
fn find_only_at_citation_placeholders() {
    // As long as placeholders are related to cites, they are found, independently of whether they
    // are valid or not
    let plhs = find_at_placeholders(DUMMY_TEXT_WITH_A_VALID_AT_CITE_PLACEHOLDER);
    let mut items = 0;
    for plh in plhs {
        match plh.placeholder_type {
            Cite(_) => items += 1,
            AtCite(_) => items += 1,
        };
    }
    assert_eq!(items, 1);
}

use std::env;
#[test]
fn check_config_attributes() {
    // Check config with default values is returned when an empty config is passed in a toml table!!!
    let t: Table = Table::new();
    match Config::build_from(Some(&t), PathBuf::new()) {
        Ok(config) => {
            println!("{:?}", config);
            assert_eq!(config.title, "Bibliography");
            assert_eq!(config.bibliography, None);
            assert_eq!(config.zotero_uid, None);
            assert!(config.cited_only);
            let default_tpl = format!("\n\n{}\n\n", DEFAULT_HB_TEMPLATE);
            assert_eq!(config.bib_hb_html, default_tpl);
            let default_css = format!("<style>{}</style>\n\n", DEFAULT_CSS_TEMPLATE);
            assert_eq!(config.css_html, default_css);
            let default_js = format!(
                "<script type=\"text/javascript\">\n{}\n</script>\n\n",
                DEFAULT_JS_TEMPLATE
            );
            assert_eq!(config.js_html, default_js);
        }
        Err(_) => panic!("there's supposed to be always a config!!!"),
    }

    // Check config attributes are processed (those that are not specified are ignored)!!!
    let mut t: Table = Table::new();

    t.insert(
        "bibliography".to_string(),
        Value::String("biblio.bib".to_string()),
    );
    t.insert(
        "zotero-uid".to_string(),
        Value::String("123456".to_string()),
    );
    t.insert("title".to_string(), Value::String("References".to_string()));
    t.insert("render-bib".to_string(), Value::String("all".to_string()));
    t.insert(
        "not-specified-config-attr".to_string(),
        Value::String("uhg???".to_string()),
    );
    match Config::build_from(Some(&t), PathBuf::new()) {
        Ok(config) => {
            println!("{:?}", config);
            assert_eq!(config.title, "References");
            assert_eq!(config.bibliography, Some("biblio.bib"));
            assert_eq!(config.zotero_uid, Some("123456"));
            assert!(!config.cited_only);
        }
        Err(_) => panic!("there's supposed to be always a config!!!"),
    }

    // Intentionally add a failure specifying a non-existing value for render-bib
    let mut t: Table = Table::new();
    t.insert(
        "render-bib".to_string(),
        Value::String("non-existent!".to_string()),
    );
    match Config::build_from(Some(&t), PathBuf::new()) {
        Ok(_) => panic!("there's supposed to be a failure in the config!!!"),
        Err(_) => println!("Yayyyyy! A failure that is supposed to happen!"),
    }

    // Test adhoc template and style!!! (We check the template and style provided for the project doc/manual)
    let mut t: Table = Table::new();
    t.insert(
        "hb-tpl".to_string(),
        Value::String("render/my_references.hbs".to_string()),
    );
    t.insert(
        "css".to_string(),
        Value::String("render/my_style.css".to_string()),
    );
    // TODO No adhoc js tested at this time. Add one if added in the future to the project manual.
    let mut manual_src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manual_src_path.push("manual/src/");
    match Config::build_from(Some(&t), manual_src_path) {
        Ok(config) => {
            println!("{:?}", config);
            let adhoc_tpl = format!("\n\n{}\n\n", EXAMPLE_HB_TEMPLATE);
            assert_eq!(config.bib_hb_html, adhoc_tpl);
            let adhoc_css = format!("<style>{}</style>\n\n", EXAMPLE_CSS_TEMPLATE);
            assert_eq!(config.css_html, adhoc_css);
            let default_js = format!(
                "<script type=\"text/javascript\">\n{}\n</script>\n\n",
                DEFAULT_JS_TEMPLATE
            );
            assert_eq!(config.js_html, default_js);
        }
        Err(e) => panic!(
            "there's supposed to be always a config!!!\n {:?}",
            e.root_cause()
        ),
    }
}

#[test]
fn check_date_extractions_from_biblatex() {
    let mut fake_bib_entry: HashMap<String, String> = HashMap::new();

    // Check when no date and no year/month we return the standard Non Available string
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "N/A");
    assert_eq!(month, "N/A");

    // Check date is split properly
    fake_bib_entry.insert("date".to_string(), "2021-02-21".to_string());
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "2021");
    assert_eq!(month, "02");

    // Check date is split properly
    fake_bib_entry.insert("date".to_string(), "2021".to_string());
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "2021");
    assert_eq!(month, "N/A");

    // Check date takes precedence over year/month
    fake_bib_entry.clear();
    fake_bib_entry.insert("date".to_string(), "2020-03".to_string());
    fake_bib_entry.insert("year".to_string(), "2021".to_string());
    fake_bib_entry.insert("month".to_string(), "jul".to_string());
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "2020");
    assert_eq!(month, "03");

    // Check year and month work too
    fake_bib_entry.clear();
    fake_bib_entry.insert("year".to_string(), "2021".to_string());
    fake_bib_entry.insert("month".to_string(), "jul".to_string());
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "2021");
    assert_eq!(month, "jul");

    // Check only month works too
    fake_bib_entry.clear();
    fake_bib_entry.insert("month".to_string(), "jul".to_string());
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "N/A");
    assert_eq!(month, "jul");

    // Check only year works too
    fake_bib_entry.clear();
    fake_bib_entry.insert("year".to_string(), "2021".to_string());
    let (year, month) = extract_date(&fake_bib_entry);
    assert_eq!(year, "2021");
    assert_eq!(month, "N/A");
}

pub struct NotFound;

/// Check if a string is present in the file contents
pub fn find_str_in_file(input: &str, file: PathBuf) -> Result<(), NotFound> {
    let text = std::fs::read_to_string(file).unwrap();

    for line in text.lines() {
        if line.contains(input) {
            return Ok(());
        }
    }
    anyhow::private::Err(NotFound)
}

#[test]
fn process_test_book() {
    let mut manual_src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manual_src_path.push("test_book/");
    let mut md = MDBook::load(manual_src_path).unwrap();
    let mdbook_bib_prepro = Bibiography::default();
    md.with_preprocessor(mdbook_bib_prepro);
    md.build().unwrap();

    // Check both, root level and nested html files get placeholders substitued with
    // bib references with relative paths
    let mut book_dest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    book_dest_path.push("test_book/public");

    let bib_reference = "bibliography.html#mdBook";

    let mut non_nested_html = book_dest_path.clone();
    non_nested_html.push("intro.html");
    match find_str_in_file(bib_reference, non_nested_html) {
        Ok(_) => (),
        Err(_) => panic!(),
    }

    let mut nested_html = book_dest_path.clone();
    nested_html.push("chapter_1/intro.html");
    match find_str_in_file(bib_reference, nested_html) {
        Ok(_) => (),
        Err(_) => panic!(),
    }
}
