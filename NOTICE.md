# Notices

## Wenmar Open

Copyright (c) 2026 Wenmar Pro. Licensed under the MIT License; see [LICENSE](LICENSE).

"Wenmar" and "Wenmar Pro" are names of Wenmar Pro. The MIT License covers the code in this repository. It does not grant permission to use those names to endorse or promote a derived product.

## Vehicle data

Decoding data is built from the Product Information Catalog and Vehicle Listing (vPIC), published by the US National Highway Traffic Safety Administration (NHTSA) at <https://vpic.nhtsa.dot.gov/>.

Wenmar Open is not affiliated with or endorsed by NHTSA. The data is provided as manufacturers reported it and comes with no warranty of accuracy or completeness. Do not rely on it as the sole source for safety-critical decisions; confirm against the vehicle and the manufacturer's service information.

## Datasets and their licences

Every dataset Wenmar Open serves, where it comes from, and what its licence asks of anyone who republishes it. The website shows the same table at `/data`. A test in `crates/open-server/src/site/licences.rs` fails when the two differ, so add a row in both places in the same pull request.

| Dataset | Source | Licence | Required attribution |
| --- | --- | --- | --- |
| VIN decoding data and vehicle catalog | NHTSA vPIC, the Product Information Catalog and Vehicle Listing, https://vpic.nhtsa.dot.gov/ | Public domain (United States government work) | None required. This site credits NHTSA. |
| Popular makes, aliases, trim spellings and added trims | Written by hand for this project: data/catalog/makes.yaml, data/catalog/names.yaml and data/presets.yaml | MIT | Keep the copyright notice in LICENSE. |
| Canadian sales tax and tire fee rates | Government and stewardship pages, each named on the calculator page, collected in data/rates/canada.toml | MIT for the collection. A rate is a fact. | Keep the copyright notice in LICENSE. |
| Vehicle recall campaigns, United States | NHTSA recall flat files, https://static.nhtsa.gov/odi/ffdd/rcl/ | Public domain (United States government work) | None required. This site credits NHTSA. |
| Vehicle recall campaigns, Canada | Transport Canada, Vehicle Recalls Database, https://open.canada.ca/data/en/dataset/1ec92326-47ef-4110-b7ca-959fab03f96d | Open Government Licence – Canada | Contains information licensed under the Open Government Licence – Canada. |
| Vehicle defect investigations, United States | NHTSA investigations flat file, https://static.nhtsa.gov/odi/ffdd/inv/ | Public domain (United States government work) | None required. This site credits NHTSA. |
| Recall remedy progress, United States | NHTSA recall quarterly and annual report flat files, https://static.nhtsa.gov/odi/ffdd/rcl/ | Public domain (United States government work) | None required. This site credits NHTSA. |
<!-- Shop-data plans add one row per dataset above this line, and the same row in crates/open-server/src/site/licences.rs. -->

## Fonts

The website serves two typefaces from its own address. Both are licensed under the SIL Open Font License, Version 1.1. The full licence text of each is in `crates/open-server/assets/fonts/` and is served beside the font.

- **DM Sans.** Copyright 2014 The DM Sans Project Authors (https://github.com/googlefonts/dm-fonts). File: `dm-sans-latin-wght.woff2`, the Latin subset of the variable font, from the package `@fontsource-variable/dm-sans` 5.3.0. Licence: `OFL-DM-Sans.txt`.
- **JetBrains Mono.** Copyright 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono). File: `jetbrains-mono-latin-400.woff2`, the Latin subset of the regular weight, from the package `@fontsource/jetbrains-mono` 5.3.0. Licence: `OFL-JetBrains-Mono.txt`.

A subset is a Modified Version under that licence. Neither font declares a Reserved Font Name, so the names are kept. The fonts are not covered by this repository's MIT License. The share image `crates/open-server/assets/og.svg` holds outlines of DM Sans letters.

## The Wenmar mark

The icon files and the share image in `crates/open-server/assets/` show the Wenmar mark and wordmark, which belong to Wenmar Pro. They are in this repository so the website can serve them. The MIT License does not grant permission to use them for anything else.

## Adding to this file

When a pull request brings in code or data from another project, add its name, source, and license text here in the same pull request. When it brings in a dataset, also add a row to "Datasets and their licences" above and to crates/open-server/src/site/licences.rs.
