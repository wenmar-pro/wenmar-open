# Contributing to Wenmar Open

Thanks for helping. This project is maintained by a small team, so the notes below are here to keep contributions quick to review.

> The project is pre-release and the code is not in the repository yet. Until it lands, the most useful thing you can do is open an issue.

## Report a wrong decode

This is the most valuable contribution. Open a [wrong decode issue](../../issues/new/choose) with:

- The VIN. The first 11 characters are usually enough if you would rather not post the full number.
- What Wenmar Open returned.
- What the vehicle actually is, and how you know (door jamb label, build sheet, manufacturer documentation).

A VIN identifies a specific vehicle. Only post one you are comfortable making public, and never include an owner's name, plate, or address.

## Report a bug or suggest something

Use the bug report or feature request templates. For anything large, open an issue before writing code so the approach can be agreed first.

## Pull requests

- Keep each pull request to one change.
- Add or update tests for behaviour you change. Decoder changes need at least one real VIN with a known answer.
- Run formatting, lints, and tests before you push. The exact commands will be listed here once the workspace exists.
- If you bring in code or data from another project, add it to [NOTICE.md](NOTICE.md) and confirm its license allows redistribution under MIT.
- Do not add data from licensed sources such as commercial labor guides, repair manuals, or paid VIN services. Only public-domain or openly licensed data can be accepted.

## Licensing of contributions

By submitting a contribution you agree that it is licensed under the [MIT License](LICENSE), and that you have the right to submit it.

## Security issues

Do not open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).

## Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).
