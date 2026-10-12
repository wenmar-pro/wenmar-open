use open_data::shop::build::build_with;
use open_data::shop::siblings;
use rusqlite::Connection;
use wenmar_shopdata::schema;

use crate::common::{Scratch, info, rows};

/// Bulletins made up for this test: `shared` that list models 1801 and
/// 1802 for 2018, `alone` that list 1801 only, and one that lists five
/// models.
fn build(shared: usize, alone: usize) -> (Scratch, Connection) {
    let scratch = Scratch::new(&format!("siblings-{shared}-{alone}"));
    build_with(
        &scratch.core(),
        &scratch.out(),
        &scratch.curated(),
        &info(),
        |context| {
            let transaction = context.transaction;
            transaction.execute_batch(schema::BULLETINS)?;
            transaction.execute_batch(
                "INSERT INTO bulletin_year (year) VALUES (2017), (2018), (2019);
                 INSERT INTO bulletin_model VALUES
                   (1, 'FORD', 'F-150', 460, 1801, 'exact'),
                   (2, 'FORD', 'FLEX', 460, 1802, 'exact'),
                   (3, 'HONDA', 'CIVIC', 474, 1863, 'exact'),
                   (4, 'HONDA', 'ACCORD', 474, 1861, 'exact'),
                   (5, 'HONDA', 'CR-V', 474, 1865, 'exact'),
                   (6, 'FORD', 'UNKNOWN', 460, NULL, NULL);",
            )?;
            let mut id = 0;
            let mut add = |models: &[i64]| -> rusqlite::Result<()> {
                id += 1;
                transaction.execute(
                    "INSERT INTO bulletin (id, type, added) VALUES (?1, 'service_bulletin', '2020-01-01')",
                    [id],
                )?;
                for model in models {
                    transaction.execute(
                        "INSERT INTO bulletin_vehicle VALUES (?1, ?2, 2018, 2018)",
                        [id, *model],
                    )?;
                }
                Ok(())
            };
            for _ in 0..shared {
                add(&[1, 2])?;
            }
            for _ in 0..alone {
                add(&[1])?;
            }
            // Five models in one bulletin, ten times over: it claims nothing.
            for _ in 0..10 {
                add(&[1, 2, 3, 4, 5])?;
            }
            // A name that matched no model is nobody's sibling.
            add(&[1, 6])?;
            siblings::shape(context)
        },
    )
    .unwrap();
    let connection = Connection::open(scratch.out()).unwrap();
    (scratch, connection)
}

const ALL: &str =
    "SELECT model_id, year, sibling_model_id, shared, total FROM vehicle_sibling ORDER BY 1, 2, 3";

#[test]
fn five_shared_documents_that_are_a_fifth_make_a_sibling() {
    // 5 shared, and the F-150 has 5 + 15 + 1 = 21 documents: 5 of 21 is
    // over a fifth. The Flex has only the 5: 5 of 5.
    let (_scratch, connection) = build(5, 15);
    assert_eq!(
        rows(&connection, ALL),
        ["1801|2018|1802|5|21", "1802|2018|1801|5|5"]
    );
}

#[test]
fn four_shared_documents_claim_nothing() {
    let (_scratch, connection) = build(4, 0);
    assert!(rows(&connection, ALL).is_empty());
}

#[test]
fn a_small_share_claims_nothing_for_the_larger_model() {
    // 5 shared of the F-150's 5 + 30 + 1 = 36: under a fifth, so the Flex
    // is not listed for the F-150; the F-150 still is for the Flex.
    let (_scratch, connection) = build(5, 30);
    assert_eq!(rows(&connection, ALL), ["1802|2018|1801|5|5"]);
}
