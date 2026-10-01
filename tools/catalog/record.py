#!/usr/bin/env python3
"""Record NHTSA's model lists for a sample of makes and model years.

Asks NHTSA's public API for the models of each make and year below, one
request a second, and writes them to a file the catalog comparison reads.
Nothing is written unless every request succeeded.

NHTSA matches the make as a substring: asking for Ford also returns Bradford
Built. Only rows whose make is exactly the one asked for are kept.

    python3 tools/catalog/record.py OUTPUT_JSON
"""
import datetime
import json
import os
import sys
import time
import urllib.parse
import urllib.request

SOURCE = (
    "https://vpic.nhtsa.dot.gov/api/vehicles/GetModelsForMakeYear"
    "/make/{make}/modelyear/{year}?format=json"
)
MAKES = [
    "Toyota", "Honda", "Ford", "Chevrolet", "Nissan", "Hyundai", "Kia", "Jeep",
    "Ram", "GMC", "Subaru", "Volkswagen", "BMW", "Mercedes-Benz", "Mazda",
    "Dodge", "Lexus", "Tesla", "Audi", "Volvo", "Land Rover", "Pontiac",
    "Harley-Davidson", "Freightliner",
]
YEARS = [1985, 1996, 2005, 2012, 2019, 2026]
PAUSE_SECONDS = 1.0


def models(make, year):
    url = SOURCE.format(make=urllib.parse.quote(make), year=year)
    request = urllib.request.Request(url, headers={"User-Agent": "wenmar-open catalog check"})
    with urllib.request.urlopen(request, timeout=120) as response:
        body = json.load(response)
    if not isinstance(body.get("Results"), list):
        sys.exit(f"NHTSA returned no result list for {make} {year}")
    names = {
        (row.get("Model_Name") or "").strip()
        for row in body["Results"]
        if (row.get("Make_Name") or "").strip().lower() == make.lower()
    }
    return sorted(name for name in names if name)


if len(sys.argv) != 2:
    sys.exit(__doc__)
output_path = sys.argv[1]

answers = []
pairs = [(make, year) for make in MAKES for year in YEARS]
for done, (make, year) in enumerate(pairs, start=1):
    answers.append({"make": make, "year": year, "models": models(make, year)})
    print(f"{done} of {len(pairs)}: {make} {year}", flush=True)
    time.sleep(PAUSE_SECONDS)

today = datetime.datetime.now(datetime.timezone.utc).date()
partial_path = output_path + ".partial"
with open(partial_path, "w") as handle:
    json.dump(
        {"recorded": today.isoformat(), "source": SOURCE, "answers": answers},
        handle,
        indent=1,
    )
    handle.write("\n")
os.replace(partial_path, output_path)
print(f"{len(answers)} answers written to {output_path}")
