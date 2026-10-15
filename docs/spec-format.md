# Spec and import formats

## Eitri spec (JSON)

**Export spec** in the Plan tab writes this file; **Import CSV / JSON** reads it back into any
project.

```json
{
  "format": "eitri-spec/1",
  "name": "Potion Icons",
  "idea": "40 fantasy potion bottles for an inventory screen",
  "style_brief": "Chunky hand-painted icons with thick outlines.",
  "shared_tail": "Framing: ...\nStyle: ...\nBackground: fully transparent ...",
  "size": "1024x1024",
  "background": "transparent",
  "gate": "figure",
  "items": [
    { "slug": "red-healing-potion", "title": "Red healing potion", "subject": "A round flask ..." }
  ]
}
```

Every image's prompt is `subject` + a blank line + `shared_tail`.

## Bare JSON list

```json
[
  { "title": "Red potion", "prompt": "A round red flask with a cork" },
  { "slug": "blue-potion", "subject": "A tall blue bottle" }
]
```

`prompt`, `subject` and `description` are interchangeable.

## CSV

A header row is required. Columns are matched by name, ignoring case:

| Meaning | Accepted column names |
| --- | --- |
| Description (required) | `subject`, `prompt`, `description`, `text` |
| File name | `slug`, `id`, `name`, `file` (a file extension is dropped) |
| Title | `title`, `label` |

Quoted fields, doubled quotes and line breaks inside quotes are supported, so a CSV whose prompt
column holds full multi-line prompts imports as is. In that case leave the project's shared
prompt empty.

Importing replaces the project's unstarted images; images that already have attempts are kept.
