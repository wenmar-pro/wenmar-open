#!/usr/bin/env python3
"""Generate the parity corpus: VINs that describe real models, built from vPIC patterns.

For a sampled model pattern, other patterns of the same schema are merged in, a
model year inside the schema's range is chosen, unconstrained positions are
filled with characters that schema's patterns use, and the check digit is
computed. The serial is 000001 wherever the patterns leave it free, so no VIN
here belongs to a real vehicle. A few VINs already published as examples are
added at the end.

Sampling uses a fixed seed over rows read in a fixed order, so the same data
file always gives the same corpus.

    python3 tools/corpus/generate.py DATA_FILE OUTPUT_JSON
"""
import collections
import json
import random
import sqlite3
import sys

SEED = 20260930
PUBLISHED_EXAMPLES = [
    "1HGCM82633A004352",
    "KM8K2CAB4PU001140",
    "5YJSA1E26HF000001",
    "1M8GDM9AXKP042788",
]
POPULAR_MAKES = [
    "Toyota", "Honda", "Ford", "Chevrolet", "Nissan", "Hyundai", "Kia", "Jeep",
    "Ram", "GMC", "Subaru", "Volkswagen", "BMW", "Mercedes-Benz", "Mazda",
    "Tesla", "Dodge", "Lexus", "Audi", "Acura", "Chrysler", "Buick",
    "Cadillac", "Volvo", "Mitsubishi",
]
HEAVY_TYPES = [
    "Truck", "Motorcycle", "Trailer", "Bus", "Incomplete Vehicle",
    "Low Speed Vehicle (LSV)", "Off-Road Vehicle",
]
# Elements merged into a VIN so it decodes to more than a model.
DETAIL_ELEMENTS = (38, 34, 5, 15, 13, 11, 12, 9, 24, 31, 75, 37, 14)
MODEL_ELEMENT = 28
FIRST_YEAR = 1981
YEAR_CODES = "ABCDEFGHJKLMNPRSTVWXY123456789"
VIN_CHARACTERS = set("ABCDEFGHJKLMNPRSTUVWXYZ0123456789")
KEY_LENGTH = 14  # positions 4-8, a pipe, positions 10-17
PIPE = 5
POSITION_7 = 3
YEAR_POSITION = 6
TRANSLITERATION = {
    **{str(digit): digit for digit in range(10)},
    **dict(zip("ABCDEFGH", range(1, 9))),
    **dict(zip("JKLMN", range(1, 6))),
    "P": 7,
    "R": 9,
    **dict(zip("STUVWXYZ", range(2, 10))),
}
WEIGHTS = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2]


def check_digit(vin):
    remainder = sum(TRANSLITERATION[c] * w for c, w in zip(vin, WEIGHTS)) % 11
    return "X" if remainder == 10 else str(remainder)


def tokens(keys):
    """One entry per position: None for `*`, else the set of characters allowed."""
    parsed, index = [], 0
    while index < len(keys):
        character = keys[index]
        if character == "*":
            parsed.append(None)
            index += 1
        elif character == "[":
            end = keys.find("]", index)
            body = keys[index + 1:end] if end > 0 else ""
            if not body or body.startswith("^"):
                return None
            allowed, at = set(), 0
            while at < len(body):
                if at + 2 < len(body) and body[at + 1] == "-":
                    allowed |= {chr(code) for code in range(ord(body[at]), ord(body[at + 2]) + 1)}
                    at += 3
                else:
                    allowed.add(body[at])
                    at += 1
            parsed.append(allowed)
            index = end + 1
        else:
            parsed.append({character})
            index += 1
    return parsed


def merge(constraints, keys):
    """Narrow per-position constraints by a pattern, or None if they conflict."""
    parsed = tokens(keys)
    if parsed is None or len(parsed) > KEY_LENGTH:
        return None
    merged = list(constraints)
    for position, allowed in enumerate(parsed):
        if allowed is None:
            continue
        if position == PIPE:
            if allowed != {"|"}:
                return None
            continue
        allowed = allowed & VIN_CHARACTERS
        merged[position] = allowed if merged[position] is None else merged[position] & allowed
        if not merged[position]:
            return None
    return merged


class Corpus:
    def __init__(self, path):
        self.database = sqlite3.connect(path)
        self.random = random.Random(SEED)
        built_at = self.database.execute("SELECT value FROM meta WHERE key = 'built_at'").fetchone()[0]
        self.latest_year = int(built_at[:4])
        self.entries = []
        self.seen_models = set()

    def schema_patterns(self, schema_id):
        return self.database.execute(
            "SELECT keys, element_id FROM pattern WHERE schema_id = ? ORDER BY id", (schema_id,)
        ).fetchall()

    def used_characters(self, patterns):
        """Characters this schema's patterns use at each position: safe fillers."""
        used = [set() for _ in range(KEY_LENGTH)]
        for keys, _ in patterns:
            parsed = tokens(keys)
            if parsed is None or len(parsed) > KEY_LENGTH:
                continue
            for position, allowed in enumerate(parsed):
                if allowed and position != PIPE:
                    used[position] |= allowed & VIN_CHARACTERS
        return used

    def build(self, code, light, schema_id, year_from, year_to, model_keys, years=None):
        constraints = merge([None] * KEY_LENGTH, model_keys)
        if constraints is None:
            return None
        patterns = self.schema_patterns(schema_id)
        details = [row for row in patterns if row[1] in DETAIL_ELEMENTS]
        self.random.shuffle(details)
        merged_elements = set()
        for keys, element in details:
            if element in merged_elements:
                continue
            narrowed = merge(constraints, keys)
            if narrowed is not None:
                constraints = narrowed
                merged_elements.add(element)
        used = self.used_characters(patterns)

        first = max(year_from, FIRST_YEAR)
        last = min(year_to or self.latest_year, self.latest_year)
        candidates = [year for year in (years or range(first, last + 1)) if first <= year <= last]
        self.random.shuffle(candidates)
        for year in candidates:
            code_for_year = YEAR_CODES[(year - 1980) % 30]
            if constraints[YEAR_POSITION] is not None and code_for_year not in constraints[YEAR_POSITION]:
                continue
            key = []
            for position in range(KEY_LENGTH):
                if position == PIPE:
                    key.append("|")
                    continue
                if position == YEAR_POSITION:
                    key.append(code_for_year)
                    continue
                allowed = constraints[position] or used[position] or set()
                if position == POSITION_7 and light:
                    # Cars, MPVs and light trucks: a digit means 1980-2009.
                    allowed = {c for c in (allowed or VIN_CHARACTERS) if c.isdigit() == (year < 2010)}
                if position >= 8 and not constraints[position]:
                    # Serial 000001 unless a pattern pins the position.
                    key.append("1" if position == KEY_LENGTH - 1 else "0")
                    continue
                if not allowed:
                    if position == POSITION_7 and light:
                        key = None
                        break
                    allowed = {"A"}
                key.append(sorted(allowed)[0])
            if key is None:
                continue
            vds, vis = "".join(key[:PIPE]), key[PIPE + 1:]
            if len(code) == 6:
                # Low-volume manufacturers: positions 12-14 complete the code.
                vis[2:5] = list(code[3:6])
            vin = code[:3] + vds + "0" + "".join(vis)
            if len(vin) != 17 or any(c not in VIN_CHARACTERS for c in vin):
                return None
            return vin[:8] + check_digit(vin) + vin[9:]
        return None

    def model_rows(self, where, parameters=()):
        return self.database.execute(
            f"""SELECT w.code, w.light_vehicle, w.vehicle_type, s.schema_id, s.year_from, s.year_to,
                       p.keys, p.value
                FROM wmi w
                JOIN wmi_schema s ON s.wmi = w.code
                JOIN pattern p ON p.schema_id = s.schema_id AND p.element_id = {MODEL_ELEMENT}
                WHERE {where}
                ORDER BY w.code, s.schema_id, s.year_from, p.id""",
            parameters,
        ).fetchall()

    def add(self, label, rows, wanted, years_for=None):
        rows = list(rows)
        self.random.shuffle(rows)
        added = 0
        for code, light, vehicle_type, schema_id, year_from, year_to, keys, model in rows:
            if added >= wanted:
                break
            if (code, model) in self.seen_models:
                continue
            years = years_for(code) if years_for else None
            vin = self.build(code, bool(light), schema_id, year_from, year_to, keys, years)
            if vin:
                self.seen_models.add((code, model))
                self.entries.append((vin, label, vehicle_type or "?"))
                added += 1
        return added

    def both_cycle_years(self, code):
        """Years this code has a schema for where the year 30 earlier has one too."""
        covered = set()
        for year_from, year_to in self.database.execute(
            "SELECT year_from, year_to FROM wmi_schema WHERE wmi = ?", (code,)
        ):
            covered |= set(range(max(year_from, FIRST_YEAR), min(year_to or self.latest_year, self.latest_year) + 1))
        later = {year for year in covered if year - 30 in covered}
        return sorted(later | {year - 30 for year in later})


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    corpus = Corpus(sys.argv[1])
    for make in POPULAR_MAKES:
        corpus.add(make, corpus.model_rows("p.make = ? COLLATE NOCASE AND w.light_vehicle = 1", (make,)), 10)
    corpus.add("light vehicles", corpus.model_rows("w.light_vehicle = 1"), 60)
    corpus.add("light trucks", corpus.model_rows("w.vehicle_type = 'Truck' AND w.light_vehicle = 1"), 40)
    for vehicle_type in HEAVY_TYPES:
        rows = corpus.model_rows("w.vehicle_type = ? AND w.light_vehicle = 0", (vehicle_type,))
        corpus.add(vehicle_type, rows, 15)
    # Where NHTSA's rule for choosing between two model years is decided:
    # heavy vehicles whose manufacturer code has schemas in both 30-year cycles.
    heavy = corpus.model_rows("w.light_vehicle = 0 AND w.vehicle_type IN ('Truck', 'Bus', 'Incomplete Vehicle', 'Motorcycle')")
    corpus.add("heavy, both cycles", heavy, 60, years_for=corpus.both_cycle_years)

    vins = sorted({vin for vin, _, _ in corpus.entries} | set(PUBLISHED_EXAMPLES))
    with open(sys.argv[2], "w") as handle:
        handle.write("[\n" + ",\n".join(json.dumps(vin) for vin in vins) + "\n]\n")
    print(f"{len(vins)} VINs written to {sys.argv[2]}")
    by_type = collections.Counter(vehicle_type for _, _, vehicle_type in corpus.entries)
    for vehicle_type, count in by_type.most_common():
        print(f"  {count:4} {vehicle_type}")


if __name__ == "__main__":
    main()
