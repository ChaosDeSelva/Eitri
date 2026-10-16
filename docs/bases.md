# Base library

Most image prompts in a set have two halves: a **generic base** that fixes the look (framing,
style, canvas, background, things to avoid) and a **specific subject** for each image. Eitri keeps
the generic half in a library so you can reuse it across projects.

## What a base holds

- **Name** and **Good for**: what the base is and which sets it suits.
- **Style brief**: two or three sentences for people.
- **Shared prompt**: the text appended to every image's subject.
- **Canvas, background and composition check** defaults.
- **Style references**: optional images that new images are drawn to match.

## Getting bases

| How | Where |
| --- | --- |
| Built-in starters | Shipped with the app: cel-shaded portrait bust, mascot crest, game item icon, pixel-art sprite, isometric tile, character full-body. Read-only; **Duplicate** to make your own version. |
| Write one | **Base library > New base**. |
| Generate one | **Base library > Generate base**: describe the look and your default planning model writes the base, streamed as it is written. It never describes a particular subject, so it works for any image in a set. |
| Save from a project | **Plan > Save as base** (next to the shared prompt) turns a project's tuned style, including its references, into a library entry. |
| Import | **Base library > Import** reads an exported `.eitri-base.json`. |

## Using a base

In a project's **Plan** tab, choose a base and press **Apply**. The project gets its own **copy** of
the style brief, shared prompt, canvas defaults and references, and remembers which base it came
from. Editing the base later never changes a project that already used it, which keeps approved
specs frozen.

With **Keep this style when drafting** checked (the default once a base is applied), the planner is
given the base's style brief and shared prompt and only writes the per-image descriptions. Eitri
also restores the exact base text afterwards, so the model cannot paraphrase it.

**New project from this base** on a base's page does both steps at once.

## File format

```json
{
  "format": "eitri-base/1",
  "name": "Chunky item icons",
  "description": "Inventory items and loot.",
  "style_brief": "Hand-painted game icons with a dark outline.",
  "shared_tail": "Framing: ...\nStyle: ...\nCanvas: 1024 by 1024 pixels.\nBackground: ...\nNo text, ...",
  "size": "1024x1024",
  "background": "transparent",
  "gate": "figure"
}
```

Reference images are not included in the file.
