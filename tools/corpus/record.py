#!/usr/bin/env python3
"""Record NHTSA's own answers for the parity corpus.

Asks NHTSA's public batch decoder about every VIN in the list, 50 at a time,
and writes the non-empty fields of each answer. Nothing is written unless every
VIN got an answer.

    python3 tools/corpus/record.py VIN_LIST_JSON OUTPUT_JSON
"""
import datetime
import json
import sys
import time
import urllib.parse
import urllib.request

SOURCE = "https://vpic.nhtsa.dot.gov/api/vehicles/DecodeVINValuesBatch/"
BATCH = 50
DROPPED = {
    "VIN", "ErrorText", "AdditionalErrorText", "PossibleValues", "SuggestedVIN",
    "VehicleDescriptor", "MakeID", "ModelID", "ManufacturerId",
}

if len(sys.argv) != 3:
    sys.exit(__doc__)
vin_list_path, output_path = sys.argv[1], sys.argv[2]
with open(vin_list_path) as handle:
    vins = json.load(handle)

answers = {}
for start in range(0, len(vins), BATCH):
    chunk = vins[start:start + BATCH]
    body = urllib.parse.urlencode({"format": "json", "data": ";".join(chunk)}).encode()
    with urllib.request.urlopen(urllib.request.Request(SOURCE, data=body), timeout=120) as response:
        results = json.load(response)["Results"]
    for result in results:
        answers[result["VIN"]] = {
            key: value.strip()
            for key, value in sorted(result.items())
            if key not in DROPPED and isinstance(value, str) and value.strip()
            and value.strip().lower() != "not applicable"
        }
    print(f"{min(start + BATCH, len(vins))} of {len(vins)}", flush=True)
    time.sleep(1)

unanswered = [vin for vin in vins if vin not in answers]
if unanswered:
    sys.exit(f"NHTSA returned no answer for {len(unanswered)} VINs, for example {unanswered[:3]}")

today = datetime.datetime.now(datetime.timezone.utc).date()
with open(output_path, "w") as handle:
    json.dump(
        {
            "recorded": today.isoformat(),
            "recorded_year": today.year,
            "source": SOURCE,
            "vins": {vin: answers[vin] for vin in sorted(vins)},
        },
        handle,
        indent=1,
        sort_keys=True,
    )
    handle.write("\n")
print(f"{len(vins)} answers written to {output_path}")
