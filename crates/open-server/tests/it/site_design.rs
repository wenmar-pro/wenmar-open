//! How the pages look, as far as text can say: the wordmark, the one red
//! action, the rules that keep a page readable at 320 pixels, in print and
//! in the dark.

use axum::http::StatusCode;

use crate::common::{self, assert_basics, assert_head, assert_targets, page};

/// One page of every kind that exists at this point of the plan.
const PAGES: [&str; 11] = [
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
            assert!(width.ends_with('%'), "a fixed width: {declaration}");
        }
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
    assert!(print.contains(
        ".skip,.by,.top nav,.crumbs,.actions,form,.screen,.pro,.more{display:none!important}"
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
    // Seen in a browser at 320 pixels: the five navigation links need 221
    // pixels and have 272, so the space between them must be under 12.
    let css = stylesheet().await;
    assert!(css.contains(
        "@media (max-width:599px){.top nav{flex-basis:100%;margin-left:0;justify-content:space-between;gap:0 8px}}"
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
