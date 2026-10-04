//! How the pages look, as far as text can say: the wordmark, the one red
//! action, the rules that keep a page readable at 320 pixels, in print and
//! in the dark.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_head, assert_targets, page};

/// One page of every kind that exists at this point of the plan.
const PAGES: [&str; 13] = [
    "/",
    "/makes",
    "/makes/honda",
    "/makes/honda/civic/2019",
    "/wmi/KM8",
    "/docs",
    "/data",
    "/about",
    "/vin/KM8K2CAB4PU001140",
    "/vin/KM8K2",
    "/tools",
    "/tools/parts-matrix",
    "/nothing",
];

async fn stylesheet() -> String {
    let app = common::app().await;
    let (status, css) = page(&app, "/assets/site.css").await;
    assert_eq!(status, StatusCode::OK);
    css
}

#[tokio::test]
async fn every_page_has_the_wordmark_and_what_a_shared_link_needs() {
    let app = common::app().await;
    for path in PAGES {
        let (_, html) = page(&app, path).await;
        assert_head(&html, path);
        assert_basics(&html, path);
        // The address bar takes the page's colour in each scheme.
        assert!(html.contains(
            r##"<meta name="theme-color" media="(prefers-color-scheme: light)" content="#f8f7f4">"##
        ));
        assert!(html.contains(
            r##"<meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0e0e10">"##
        ));
    }
    // The fixed-width face is asked for early only where it is on the first
    // screen.
    let mono = r#"<link rel="preload" href="/assets/fonts/jetbrains-mono-latin-400.woff2" as="font" type="font/woff2" crossorigin>"#;
    for (path, early) in [
        ("/", true),
        ("/vin/KM8K2CAB4PU001140", true),
        ("/vin/KM8K2", true),
        ("/about", false),
        ("/makes", false),
    ] {
        let (_, html) = page(&app, path).await;
        assert_eq!(html.contains(mono), early, "{path}");
    }
    // A result page is shared without its address.
    let (_, html) = page(&app, "/vin/KM8K2CAB4PU001140").await;
    let head = html.split("</head>").next().unwrap();
    assert!(!head.contains("KM8K2CAB4PU001140"), "{head}");
}

#[tokio::test]
async fn the_brand_red_marks_the_wordmark_and_one_action() {
    let css = stylesheet().await;
    // The colour itself is written once, as a token.
    assert_eq!(css.matches("#e50914").count(), 1, "the red is a token");
    // It is used by the wordmark, the primary button and the focus outline,
    // and by nothing else.
    let allowed = [
        ".name span{",
        "button.primary{",
        "button.primary:hover{",
        "a:focus-visible,button:focus-visible,input:focus-visible,select:focus-visible{",
    ];
    let mut uses = 0;
    for rule in css.split('}') {
        if !rule.contains("var(--brand") {
            continue;
        }
        uses += 1;
        let rule = rule.trim_start();
        assert!(
            allowed.iter().any(|selector| rule.starts_with(selector)),
            "red used by: {rule}"
        );
    }
    assert_eq!(uses, 4);

    let app = common::app().await;
    let (_, home) = page(&app, "/").await;
    assert_eq!(home.matches(r#"class="primary""#).count(), 1);
    assert!(home.contains(r#"<button class="primary" type="submit">Decode</button>"#));
    // A page with nothing to do has the wordmark as its only red.
    for path in ["/makes", "/about", "/docs", "/vin/KM8K2CAB4PU001140"] {
        let (_, html) = page(&app, path).await;
        assert_eq!(html.matches(r#"class="primary""#).count(), 0, "{path}");
    }
}

#[tokio::test]
async fn long_words_wrap_and_wide_things_scroll() {
    let css = stylesheet().await;
    // A make or a trim with a long name and no space breaks where it must
    // instead of pushing the page wider than the screen.
    assert!(css.contains("overflow-wrap:anywhere"), "on the body");
    assert!(css.contains("table{width:100%;table-layout:fixed;"));
    // Code keeps its lines and scrolls inside its own box.
    assert!(css.contains("pre{") && css.contains("overflow-x:auto;overflow-wrap:normal"));
    // The VIN box's text shrinks with the screen and never below 18 pixels.
    assert!(css.contains("form.vin input{min-height:56px;font-size:clamp(18px,5.6vw,24px)}"));
    // Nothing has a fixed width wider than a small phone.
    for declaration in css.split(['{', '}', ';']) {
        if let Some(width) = declaration.trim().strip_prefix("width:") {
            assert!(
                width.ends_with('%') || width == "auto",
                "a fixed width: {declaration}"
            );
        }
    }
    // A table whose last column is sentences sizes its other columns to
    // what they hold, on one line, and the sentences get the rest: equal
    // thirds leave them 95 pixels on a phone.
    assert!(css.contains("table.prose{table-layout:auto}"));
    assert!(
        css.contains(
            "table.prose th,table.prose td:not(:last-child){width:auto;white-space:nowrap}"
        )
    );
    let guides = common::app().await;
    let (_, html) = page(&guides, "/guides/how-to-read-a-vin").await;
    assert_eq!(
        html.matches(r#"<table class="prose">"#).count(),
        1,
        "{html}"
    );
    // Columns of a character or a year each stay as they are.
    for path in ["/guides/model-year", "/guides/check-digit"] {
        let (_, html) = page(&guides, path).await;
        assert!(
            html.contains("<table>") && !html.contains("class=\"prose\""),
            "{path}"
        );
    }
    // The name used in the fixture for this is real: a page with it renders.
    let app = common::app_with_rows(
        "INSERT INTO catalog_make VALUES (8100, 'long', 'Supercalifragilisticexpialidocious-Manufacturing-And-Coachworks-Of-Saskatchewan-Incorporated', 'long', NULL, 4, 1);
         INSERT INTO catalog_model VALUES (9900, 8100, 'longmodel', 'Extraordinarily-Long-Model-Name-With-No-Spaces-At-All-2019-Limited-Edition', 'longmodel', 2019, 2019, 4, 1);
         INSERT INTO catalog_vehicle VALUES (30, 2019, 8100, 9900, 4, 1, NULL);",
    )
    .await;
    for path in ["/makes", "/makes/long", "/makes/long/longmodel/2019"] {
        let (status, html) = page(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(html.contains("Coachworks-Of-Saskatchewan"), "{path}");
    }
}

#[tokio::test]
async fn the_stylesheet_loads_only_the_sites_own_fonts() {
    let css = stylesheet().await;
    assert!(!css.contains("@import"));
    let mut loaded = 0;
    for address in css.split("url(").skip(1) {
        loaded += 1;
        assert!(address.starts_with("/assets/fonts/"), "{:.60}", address);
    }
    assert_eq!(loaded, 2);
    assert_eq!(css.matches("font-display:swap").count(), 2);
    assert!(css.contains("src:url(/assets/fonts/dm-sans-latin-wght.woff2) format(\"woff2\")"));
    assert!(
        css.contains("src:url(/assets/fonts/jetbrains-mono-latin-400.woff2) format(\"woff2\")")
    );
    // While a font loads, or if it never does, the text is set in a face
    // the visitor has, sized to match.
    assert!(
        css.contains("font-family:\"DM Sans Fallback\";src:local(\"Arial\");size-adjust:104.53%")
    );
    // The licences are named where the fonts are declared.
    assert!(css.contains("/assets/fonts/OFL-DM-Sans.txt"));
    assert!(css.contains("/assets/fonts/OFL-JetBrains-Mono.txt"));
    assert!(css.len() < 9_000, "the stylesheet is {} bytes", css.len());
}

#[tokio::test]
async fn print_is_black_on_white_whatever_the_scheme() {
    let css = stylesheet().await;
    let print = css.split("@media print{").nth(1).expect("print rules");
    for token in [
        "color-scheme:light",
        "--page:#fff",
        "--surface:#fff",
        "--heading:#000",
        "--body:#000",
        "--secondary:#333",
        // The wordmark is one colour where colour is not available.
        "--brand:#000",
    ] {
        assert!(print.contains(token), "{token}");
    }
    // `!important`, because a selector such as `ul.plain` outweighs `.screen`
    // and would put the links list of a result page back on paper.
    // A calculator's form is the one form that is printed: its fields are
    // the inputs the result was worked out from.
    assert!(print.contains(
        ".skip,.by,.top nav,.crumbs,.actions,form:not(.calc),.screen,.pro,.more{display:none!important}"
    ));
    // Nothing outside the print rules sets `display` with `!important`, which
    // would outweigh that rule in turn, except the `hidden` attribute.
    let screen = css.split("@media print{").next().unwrap();
    assert_eq!(screen.matches("!important").count(), 1);
    assert!(screen.contains("[hidden]{display:none!important}"));
    // The wordmark and the data version line up with the tables.
    assert!(print.contains(".top,footer{padding-left:0;padding-right:0}"));
    // What a result page hides on paper carries one of those names.
    let app = common::app().await;
    let (_, html) = page(&app, "/vin/KM8K2CAB4PU001140").await;
    assert!(html.contains(r#"<ul class="plain links screen">"#));
    assert!(html.contains(r#"<p class="pro">"#));
    assert!(html.contains(r#"<div class="actions">"#));
}

#[tokio::test]
async fn a_small_phone_gets_two_header_rows_and_whole_words() {
    // Measured in a browser at 320 pixels: the six navigation links need
    // 261 pixels at the text size and have 272, which leaves no room
    // between them. At the small size they need 228, so the space between
    // them must be under 8.
    let css = stylesheet().await;
    assert!(css.contains(
        "@media (max-width:599px){.top nav{flex-basis:100%;margin-left:0;justify-content:space-between;gap:0 6px;font-size:var(--small)}}"
    ));
    // "Displacement," is 106 pixels wide and the label column of a spec
    // sheet is 270 pixels times this, less 8 of padding.
    assert!(css.contains("tbody th{width:44%;"));
    // A link that wraps to a second line keeps clear of the next one.
    assert!(css.contains("ul.plain a{padding:6px 0}"));
}

#[tokio::test]
async fn nothing_moves_and_colour_changes_respect_the_visitors_setting() {
    let css = stylesheet().await;
    assert_eq!(css.matches("transition").count(), 1);
    assert!(css.contains(
        "@media (prefers-reduced-motion:no-preference){a,button,input,select{transition:color 75ms,background-color 75ms,border-color 75ms}}"
    ));
    assert!(!css.contains("animation"));
    // `text-transform` sets capitals. Nothing is moved or scaled.
    assert_eq!(
        css.matches("transform").count(),
        css.matches("text-transform").count()
    );
}

#[tokio::test]
async fn whatever_can_be_tapped_is_tall_enough() {
    let css = stylesheet().await;
    assert!(css.contains(
        ".name,.by,.top nav a,footer a,ul.plain a,.more a,.crumbs a{display:inline-flex;align-items:center;min-height:var(--touch)}"
    ));
    assert!(
        css.contains("input,select,button{font:inherit;color:inherit;min-height:var(--touch);")
    );
    let app = common::app().await;
    for path in ["/", "/vin/KM8K2", "/vin/KM8K2CAB4PUO01140", "/nothing"] {
        let (_, html) = page(&app, path).await;
        assert_targets(&html, path);
    }
}

#[tokio::test]
async fn a_calculators_form_holds_plain_figures_and_marks_a_field_it_could_not_read() {
    let css = stylesheet().await;
    // The VIN box is set in capitals of the fixed-width face. A figure is
    // not: it is in the text face, as typed, with digits of one width.
    assert!(css.contains(
        "form.calc input{font-family:inherit;letter-spacing:0;text-transform:none;font-variant-numeric:tabular-nums}"
    ));
    // A field that could not be read has the edge of a warning, in the
    // warning's own colour, and its message sits close under it.
    assert!(css.contains(
        "input[aria-invalid]{border-color:var(--warn-text);box-shadow:0 0 0 1px var(--warn-text)}"
    ));
    assert!(css.contains(".field .warning{margin:4px 0 0}"));
    // No colour of its own: every colour in the rules for a form is a token.
    for rule in css
        .split('}')
        .filter(|rule| rule.contains("calc") || rule.contains("aria-invalid"))
    {
        assert!(!rule.contains('#'), "a colour outside the tokens: {rule}");
    }
}

#[tokio::test]
async fn six_header_entries_fit_a_small_phone() {
    // Measured in a browser at 320 pixels: at the text size the six links
    // are 261 pixels wide and the row is 272, so they took two rows. At
    // the small size they are 228, and six pixels between them fits.
    let css = stylesheet().await;
    assert!(css.contains("gap:0 6px;font-size:var(--small)}}"));
    // Smaller letters, and still as tall as a finger.
    assert!(css.contains(".name,.by,.top nav a,footer a,ul.plain a,.more a,.crumbs a{display:inline-flex;align-items:center;min-height:var(--touch)}"));
    let app = common::app().await;
    for path in ["/", "/tools", "/tools/parts-matrix", "/nothing"] {
        let (_, html) = page(&app, path).await;
        let nav = html.split(r#"<nav aria-label="Site">"#).nth(1).unwrap();
        let nav = nav.split("</nav>").next().unwrap();
        assert_eq!(nav.matches("<a href=").count(), 6, "{path}");
        // No entry is long enough to need a second line.
        for name in nav.split("</a>").filter_map(|link| link.rsplit('>').next()) {
            assert!(name.trim().len() <= 9, "{path}: {name}");
        }
    }
}

#[tokio::test]
async fn the_rows_of_the_matrix_form_fit_a_small_phone() {
    let css = stylesheet().await;
    // Three boxes and the row's number share the width, whatever it is:
    // a box may shrink below the width a browser gives it by default.
    assert!(css.contains(
        ".tier{display:grid;grid-template-columns:1em repeat(3,minmax(0,1fr));gap:0 8px;align-items:end}"
    ));
    // Measured at 320 pixels: a box is 75 pixels wide inside its padding,
    // and 1000.00 is shown whole.
    assert!(css.contains(".tier input{padding:8px 6px}"));
    // The labels of the rows after the first are off the screen, not gone.
    assert!(css.contains(".tier.rest label{position:absolute;left:-999px}"));
    // A message under a row has the row's whole width.
    assert!(css.contains(".tier .warning{grid-column:1/-1;margin:4px 0 0}"));
    let app = common::app().await;
    let (_, html) = page(&app, "/tools/parts-matrix").await;
    assert_eq!(html.matches(r#"<div class="tier"#).count(), 8);
    assert_eq!(html.matches(r#"<div class="tier rest"#).count(), 7);
    assert_eq!(html.matches(r#"role="group" aria-label="Row "#).count(), 8);
    // No box asks for a width of its own.
    assert!(!html.contains(" size=") && !html.contains("style="));
    // The result table scrolls inside its own box.
    assert!(html.contains(
        "<div class=\"scroll\">\n<table class=\"prose\">\n<caption>The matrix</caption>"
    ));
    assert_targets(&html, "/tools/parts-matrix");
}

#[tokio::test]
async fn a_calculator_prints_its_inputs_and_its_result_and_not_the_navigation() {
    let css = stylesheet().await;
    let print = css.split("@media print{").nth(1).expect("print rules");
    assert!(print.contains("form:not(.calc)"));
    // The class of the rows after the first is not one the print rules hide.
    assert!(print.contains(".more{display:none!important}") && !print.contains(".rest"));
    let app = common::app().await;
    let (_, html) = page(&app, "/tools/parts-matrix").await;
    // Printed: the form, with the rows that hold something.
    assert!(html.contains(r#"<form class="calc" id="calc""#));
    assert_eq!(
        html.matches(r#"<div class="tier rest screen""#).count(),
        2,
        "rows 7 and 8 of the example are empty"
    );
    // Not printed: the presets and what is said of them, the button, the
    // print button, the link to Wenmar Pro.
    assert!(html.contains(r#"<ul class="plain links screen">"#));
    assert!(html.contains(r#"<p class="note screen">"#));
    assert!(html.contains(r#"<div class="screen"><button class="primary" type="submit">"#));
    assert!(html.contains(
        "<div class=\"actions\">\n<button type=\"button\" data-print hidden>Print</button>"
    ));
    assert!(html.contains(r#"<p class="pro">"#));
    // The tools index has nothing to hide but the layout's own parts.
    let (_, index) = page(&app, "/tools").await;
    assert!(!index.contains("<form") && !index.contains("data-print"));
}
