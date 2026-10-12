# Contributing to Wenmar Open

Thanks for helping. This project is maintained by a small team, so the notes below are here to keep contributions quick to review.

> The project is pre-release. The decoder and the data build are here; the API and website are not yet.

## Report a wrong decode

This is the most valuable contribution. Open a [wrong decode issue](../../issues/new/choose) with:

- The VIN. The first 11 characters are usually enough if you would rather not post the full number.
- What Wenmar Open returned.
- What the vehicle actually is, and how you know (door jamb label, build sheet, manufacturer documentation).

A VIN identifies a specific vehicle. Only post one you are comfortable making public, and never include an owner's name, plate, or address.

## Report a bug or suggest something

Use the bug report or feature request templates. For anything large, open an issue before writing code so the approach can be agreed first.

## Pull requests

- Open pull requests against `dev`. `main` holds what has been released.
- Keep each pull request to one change.
- Add or update tests for behaviour you change. Decoder changes need at least one real VIN with a known answer.
- Run formatting, lints, and tests before you push: `bin/check` (or `mise run check`), or the three `cargo` commands listed under `[tasks.check]` in `mise.toml`. A new checkout needs `bin/setup` first. The scripts in `bin/` each wrap one `mise run` task; see the README.
- If you change decoding, run `mise run parity` against a built data file (`mise run data`) and explain any VIN that newly differs from NHTSA.
- If you change the catalog build or the lists under `data/`, run `mise run catalog-parity` against a built data file and explain any model that newly differs from NHTSA.
- Trims added to `data/presets.yaml` and spellings added to `data/catalog/names.yaml` must be your own work or public knowledge. Do not copy lists from licensed catalogs or parts databases. Every hand-written record follows "Curated data" below.
- If you bring in code or data from another project, add it to [NOTICE.md](NOTICE.md) and confirm its license allows redistribution under MIT.
- Do not add data from licensed sources such as commercial labor guides, repair manuals, or paid VIN services. Only public-domain or openly licensed data can be accepted.

## Curated data

Some of the data here is written by hand: the lists under `data/` today, and in time fluids, tire fitments, trouble-code text, links to the makers' pages and labour times. These rules apply to every such record. They are stricter than the rules for code, because a wrong figure can damage a vehicle, and a copied sentence can end the project.

### Where a record may not come from

- Not from SAE J2012 or any other standard that is sold.
- Not from a labour guide or a repair database: MOTOR, Mitchell, ALLDATA, Identifix, Autodata, or any other.
- Not from another website's, app's or parts catalogue's compiled list, whatever it says about viewing.
- Not from the wording of an owner's manual, a service manual or a bulletin. A fact read there may be recorded: a capacity, a size, a specification, an interval. Its sentences, a whole table, and its pictures may not.
- Not from memory. If you cannot point to where it is printed, do not record it.

### What a record must carry

- `source`: where the fact is printed, exactly enough for someone else to find it. For example `owner's manual, 2019 Civic, page 612`, `under-hood label`, `door placard`, or the address of the maker's or a government's own page.
- `checked`: the day you read the source, written `2026-10-12`.
- Text in your own words, where the record has text. Write it from what you know and check it against the source. Do not paraphrase the source sentence by sentence.
- Every answer, when there are several. A model year can have one answer per trim, engine or market. Record each with what it covers. Never pick one.
- The market the source is for (United States or Canada), where the file has a place for it.

### What the pull request must show

- For each record, the source, and how the reviewer can see it: a link to the page, or a photograph of the label or placard with the serial number of the VIN and any licence plate covered. Do not attach pages of a manual.
- At most 20 records, so that each can be checked.
- That `bin/check` passes. The build checks the shape of every record, not whether it is true.
- A row in "Datasets and their licences" in `NOTICE.md` and in `crates/open-server/src/site/licences.rs`, if the pull request starts a new dataset.

### Who approves

- A record is merged only after a maintainer other than its author has opened the source and compared every figure with it. Reading the diff is not enough.
- That reviewer's GitHub name goes in the record's `reviewer` field where the file has one. Where it has none, the approving review on the pull request is the record.
- If there is no second person, the record waits.
- A correction sent through "Report an error" is accepted the same way: a person edits the file in a pull request. Nothing a visitor sends changes data by itself.

## Licensing of contributions

By submitting a contribution you agree that it is licensed under the [MIT License](LICENSE), and that you have the right to submit it.

## Security issues

Do not open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).

## Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).
