use shop_math::Percent;
use shop_math::targets::{LABOR, OVERALL, PARTS, Range, Source, Standing, Target};

const WICKEDFILE: &str = "https://www.wickedfile.com/blogs/parts-vs-labor-margin-auto-repair/";
const ELITE_SIX: &str = "https://eliteworldwide.com/info-center/six-strategies-to-keep-your-auto-repair-shop-profitable/";
const ELITE_KEEPING: &str =
    "https://eliteworldwide.com/info-center/keeping-your-shop-competitive-and-profitable/";
const PARTSTECH: &str =
    "https://partstech.com/resource/blog/the-keys-to-maximizing-your-shops-profits/";

fn figures(target: &Target) -> (i64, i64, i64) {
    let whole = |percent: Percent| percent.thousandths() / 1_000;
    (
        whole(target.range.low),
        whole(target.range.high),
        whole(target.usual),
    )
}

fn urls(sources: &[Source]) -> Vec<&str> {
    sources.iter().map(|source| source.url).collect()
}

#[test]
fn the_ranges_and_usual_targets_are_those_of_the_design() {
    assert_eq!(figures(&LABOR), (60, 75, 70));
    assert_eq!(figures(&PARTS), (40, 50, 50));
    assert_eq!(figures(&OVERALL), (50, 60, 60));
}

#[test]
fn every_range_is_from_wickedfile_and_every_target_from_the_page_that_states_it() {
    for target in [&LABOR, &PARTS, &OVERALL] {
        assert_eq!(urls(target.range_sources), [WICKEDFILE]);
        assert_eq!(target.range_sources[0].name, "WickedFile");
    }
    assert_eq!(urls(LABOR.usual_sources), [ELITE_SIX, PARTSTECH]);
    assert_eq!(urls(PARTS.usual_sources), [ELITE_KEEPING]);
    assert_eq!(urls(OVERALL.usual_sources), [PARTSTECH]);
}

#[test]
fn only_the_three_companies_of_the_design_are_named() {
    for target in [&LABOR, &PARTS, &OVERALL] {
        for source in target.range_sources.iter().chain(target.usual_sources) {
            assert!(
                ["WickedFile", "Elite Worldwide", "PartsTech"].contains(&source.name),
                "{}",
                source.name
            );
            assert!(source.url.starts_with("https://"), "{}", source.url);
            assert!(!source.title.is_empty());
        }
    }
}

#[test]
fn a_figure_on_either_end_of_a_range_is_inside_it() {
    let range = Range {
        low: Percent::whole(40),
        high: Percent::whole(50),
    };
    let standing = |thousandths: i64| range.standing(Percent::from_thousandths(thousandths));
    assert_eq!(standing(39_999), Standing::Below);
    assert_eq!(standing(40_000), Standing::Inside);
    assert_eq!(standing(45_000), Standing::Inside);
    assert_eq!(standing(50_000), Standing::Inside);
    assert_eq!(standing(50_001), Standing::Above);
    assert_eq!(standing(-10_000), Standing::Below);
}

#[test]
fn each_usual_target_is_inside_its_own_range() {
    for target in [&LABOR, &PARTS, &OVERALL] {
        assert_eq!(target.standing(target.usual), Standing::Inside);
    }
    assert_eq!(LABOR.standing(Percent::whole(59)), Standing::Below);
    assert_eq!(OVERALL.standing(Percent::whole(61)), Standing::Above);
}

#[test]
fn a_standing_is_written_as_one_lowercase_word() {
    assert_eq!(Standing::Below.to_string(), "below");
    assert_eq!(Standing::Inside.to_string(), "inside");
    assert_eq!(Standing::Above.to_string(), "above");
}

/// The targets are in one place: no other source file of the crate names a
/// source, so no other file can state a target with one.
#[test]
fn no_other_file_of_the_crate_names_a_source() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut read = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap() == "targets.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap().to_lowercase();
        for name in [
            "wickedfile",
            "eliteworldwide",
            "elite worldwide",
            "partstech",
        ] {
            assert!(!text.contains(name), "{} names {name}", path.display());
        }
        read += 1;
    }
    assert!(read >= 4, "only {read} source files were read");
}
