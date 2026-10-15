# Pixel checks and background keying

The checks were tuned on a production run of several thousand transparent images, where each
rule was added after a defect shipped without it. They run on every image as it arrives.

## Modes

Pick one per project (**Composition check** in the Plan tab):

| Mode | For | Passes when |
| --- | --- | --- |
| Free-standing subject (`figure`) | icons, items, full figures that touch no edge | background is 8% to 88% of the frame and fewer than 10 see-through pixels are sealed inside the subject |
| Bust / portrait (`portrait`) | head-and-shoulders cropped by the frame | both top corners are fully transparent and fewer than 10 see-through pixels are sealed in the lower body |
| No pixel checks (`none`) | opaque art | always |

All modes: an image with all four corners above alpha 200 has a **solid background** and fails.

## How it measures

- Corner alpha is sampled 2 px in from each corner.
- The image is downscaled to 256x256. A pixel is "open" when its alpha is below 230.
- A 4-connected flood fill runs from the border (figure) or from the two top corners (portrait).
  Open pixels the flood never reaches are sealed inside the subject.
- Portrait mode only counts sealed pixels in the lower body box (x 51 to 205, y 184 to 252 on the
  256 grid), because gaps between arms and torso that open to the outside are fine.

## Verdicts

- **Pass** goes to done.
- **Look closer** (passed, but every corner is clear in portrait mode, so the subject may sit small
  in the frame; or the image is not square) goes to needs review.
- **Fail** gets one automatic retry: a solid background gets a targeted "make the background
  transparent" edit, anything else a fresh draw from the same prompt. A second failure goes to
  needs review with the reasons shown.

## Background keying

Providers without native transparency are asked for a flat chroma-key green (#00FF00) backdrop.
After the image arrives:

1. The backdrop colour is the median of the border pixels. If fewer than 80% of border pixels
   match it, the image is left alone and the checks decide.
2. A flood fill from the border clears every connected pixel within distance 70 of the backdrop,
   so green inside the subject survives.
3. Outline pixels next to the cleared area are faded by their closeness to the backdrop and
   de-spilled (green clamped to the larger of red and blue).
