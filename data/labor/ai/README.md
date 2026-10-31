# AI labour estimates

Each file here holds a model's estimates of the hours one job takes: `oil-change.yaml` for the job `oil-change`. `open-data build-shop` builds them into `labor_estimate` with basis `ai`, and how each was worked out into `labor_ai` (dataset `labor_ai`). They are published under **CC BY 4.0** with the credit `Labour estimates by Wenmar Pro, CC BY 4.0`, and every answer labels them `"basis": "ai"`.

**Nobody writes these files by hand.** Wenmar's labour generator writes them; it is not part of this repository. A new run replaces the job's file.

## The rule

A model may be given the **steps** of a repair procedure as input, read from a local directory at run time. It is never given, and a file here never holds, a labour time taken from anywhere: the generator removes every hour figure and every line about labour time from the procedure before the model sees it, and the prompt tells the model to estimate from the steps alone and never repeat a time it has seen. No procedure text is ever committed. Hand-written estimates in `data/labor/` keep every rule they had.

## Which figure a vehicle gets

As for every estimate, the narrowest scope that has one. Within one scope a measured figure comes first, then a written one, then an AI one.

## A file

```yaml
job: oil-change
method: "open-labour-ai-1"
model: "claude-opus-5-5"
generated: 2026-10-30
estimates:
  - scope: model
    make: toyota
    model: corolla
    low: "0.4"
    typical: "0.5"
    high: "0.9"
    confidence: 45
    inputs: [vehicle, job, procedure]
    notes: "What the model assumed, in its words."
```

| Field | Meaning |
|---|---|
| `method` | The prompt version (`METHOD` in the generator). |
| `model` | The model that answered. |
| `generated` | The day the file was written. |
| scope fields | As in `data/labor/README.md`. |
| `low`, `typical`, `high` | Hours in quotes, from `0.01` to `40`, in order; `high` at most 3 × `typical`. |
| `confidence` | The model's own, 0 to 60. |
| `inputs` | What it was worked out from: `vehicle`, `job`, `wenmar-figures` (Wenmar's written and measured class figures of the job), `procedure` (repair steps, numbers removed). |
| `notes` | Optional, at most 300 characters. |
| `llm`, `method` | Optional, per figure: the model that answered and the prompt version of this figure, when not the file's `model` and `method` (a file a second model, or a later prompt, added figures to). |

The build refuses any other field, a guardrail broken, a job not in `data/jobs.yaml`, a vehicle not in the catalog, or two figures of one job for one scope.

## How they are made

For each job on each target vehicle (mostly a generation of a model), the generator asks the model for a low, typical and high figure, with the job's name, what it includes and excludes, and the vehicle. The repair steps are added when there is a procedure for the pair, after every number and every line about labour time is removed. The model may answer that the job does not exist on the vehicle (a timing belt on a chain engine), and no figure is written. An answer outside the guardrails above is refused, not corrected.

Figures can be checked against a second model and against shops' billed hours; agreement with billed hours says a figure agrees with what shops charge, not with a clock.
