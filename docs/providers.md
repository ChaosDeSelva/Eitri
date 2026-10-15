# Providers

Each provider has its own key slot in Settings, stored in the OS keychain. A project picks one
provider for drawing and one for planning. Model ids are free text: the lists below are presets,
and any model id the provider accepts can be typed in.

Status:

- **Verified**: used in production by the maintainers.
- **Implemented**: written from the provider's public API docs and covered by unit tests of the
  request and response shapes, but not exercised against the live service by the maintainers.
  Please report anything that breaks.

## OpenAI (verified)

- Draw: `POST /v1/images/generations`, or `POST /v1/images/edits` (multipart) when style references
  or a source image are attached.
- Plan: `POST /v1/responses` with `stream: true` and a strict JSON schema.
- Transparency: native (`background: transparent`).
- Cost: from the token usage in each response.
- Key: <https://platform.openai.com/api-keys>. Some accounts must complete organization
  verification before image models are available; the error says so.

## Google Gemini (implemented)

- Gemini image models: `POST /v1beta/models/{model}:generateContent` with
  `responseModalities: ["IMAGE", "TEXT"]`; references and edit sources are sent as inline images.
- Imagen: `POST /v1beta/models/{model}:predict`. No references.
- Plan: `:streamGenerateContent?alt=sse` with `responseMimeType: application/json` and a response
  schema (Eitri strips `additionalProperties`, which Gemini's schema subset rejects).
- Transparency: keyed.
- Key: <https://aistudio.google.com/apikey> (sent as `x-goog-api-key`).

## fal.ai (implemented)

- Draw: `POST https://fal.run/{model}` with `Authorization: Key ...`. The image comes back as a URL
  (or data URI) and is downloaded.
- Kontext models receive the first reference or the edit source as `image_url` (data URI).
- Transparency: keyed. Cost: per-image estimate.
- Any fal model id works if it accepts `prompt` and returns `images[0].url`.

## Replicate (implemented)

- Draw: `POST /v1/models/{owner}/{name}/predictions` with `Prefer: wait`, then polls `urls.get`
  until the prediction succeeds or fails.
- Kontext models receive the first reference or edit source as `input_image` (data URI).
- Transparency: keyed. Cost: per-image estimate.
- Official models with an `aspect_ratio` input work best.

## Stability AI (implemented)

- Draw: `POST /v2beta/stable-image/generate/{core|ultra|sd3}` (multipart), raw image back.
- Model ids: `core`, `ultra`, `sd3.5-large`, `sd3.5-medium`, `sd3.5-large-turbo`.
- No references or edits. Transparency: keyed. Cost: per-image estimate.

## OpenAI-compatible (implemented)

- Any server that implements `/images/generations` (and optionally `/images/edits` and
  `/responses`). Set the base URL in Settings; the key is optional.
- Useful for local models and self-hosted gateways.

## Adding a provider

1. Add `src-tauri/src/providers/<name>.rs` with a `draw(http, base, key, &Draw)` function returning
   `ImageResult` (PNG bytes and usage), and optional `verify` / `plan` / `list_models`.
2. Add a `Kind` variant and wire it in `Provider::draw`, `verify`, `default_base`.
3. Add a catalog entry with model presets, capabilities and a per-image estimate.
4. Classify HTTP failures with `classify_generic` so quota, auth and refusals behave correctly.
5. Unit-test the request body and response parsing. See `fal.rs` for a small example.
