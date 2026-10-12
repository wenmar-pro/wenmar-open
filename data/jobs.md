# Jobs

`jobs.yaml` lists the repair jobs a labour time can be about. It is written by hand for this project. A labour time is only comparable between two shops if both mean the same work by the job, so each job says what it includes and what it does not, and its id never changes once a labour record (`data/labor/`) or a repair order uses it.

Wenmar Pro records repair orders against these ids. Anyone else may use them too.

## Fields

```yaml
status: draft          # draft | reviewed
updated: 2026-10-12    # the day the list was last changed
jobs:
  - id: test-job
    name: "Test job"
    system: engine
    components: [timing-belt]
    includes: ["made-up step one", "made-up step two"]
    excludes: ["made-up exclusion"]
    related: [another-test-job]
```

| Field | Meaning |
|---|---|
| `status` | `draft` while any job is incomplete: the build checks the file and builds nothing from it. `reviewed` once a person who does the work has completed every job: the build then makes the `jobs` dataset, and every job must have at least one `includes` entry. |
| `updated` | The day of the last change, `2026-10-12`. |
| `id` | Lowercase words joined by hyphens: `brake-pads-front`. Permanent once used. To change what a job means, add a new job and leave the old id alone. |
| `name` | What a page shows: an action and its object, in our own words. At most 80 characters. |
| `system` | The id of a system in `data/components.yaml` (an entry with no `parent`). |
| `components` | Up to 8 ids from `data/components.yaml`: the parts the job acts on. They link a job to the recalls, bulletins and trouble codes about those parts. May be left out. |
| `includes` | Up to 12 short entries, each at most 120 characters: the work the job covers, in the order it is done. |
| `excludes` | Up to 12 short entries: work often done at the same visit that is **not** part of this job, and is a job of its own. |
| `related` | Up to 10 other job ids. May be left out. |

## Rules

- **Never a time.** No hours and no minutes in this file. Times are in `data/labor/`.
- **Our own words.** Do not copy a job list, an operation name or an operation description from a labour guide (MOTOR, Mitchell, ALLDATA), from shop software or from another website.
- **One job, one piece of work.** Where shops often do two things together and bill them as one, make a job for the pair as well (`brake-pads-and-rotors-front`) rather than stretching one job to mean both.
- **General, not about one make.** A job is the same job on every vehicle. What differs by vehicle is the time.

## Checking a change

```bash
cargo test -p open-data --test shop jobs::
```

The build stops if `status` is not `draft` or `reviewed`; if `updated` is not a day; if an id is not lowercase words joined by hyphens or is listed twice; if a name is empty or too long; if `system` is not a system of `components.yaml`; if a component is not in `components.yaml` or is named twice; if an entry of `includes` or `excludes` is empty or too long; if a related job is not in the list, is the job itself or is named twice; if a reviewed list has a job with no `includes`; or if the file has a field this page does not name.
