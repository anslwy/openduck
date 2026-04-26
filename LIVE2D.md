# Live2D Characters

OpenDuck can attach a Live2D Cubism model to any contact. The app expects a Cubism 4 style `.model3.zip` file containing a `.model3.json` manifest, the referenced `.moc3`, textures, and optional expression, motion, pose, physics, and display-info files.

|![Jin](/screenshots/spanish-live2d.png) | ![Haru](/screenshots/japanese-live2d.png) |
|:---:|:---:|

## Sample Characters

The bundled Live2D demo characters, Haru and Jin Natori, are based on Live2D sample data from:

https://www.live2d.com/en/learn/sample/

They are included for demonstration and SDK testing purposes only. Review Live2D's Free Material License Agreement and sample-data terms before using, redistributing, or modifying those assets.

## Add A Live2D Model To A Contact

1. Open OpenDuck and go to `Characters`.
2. Select an existing contact or create a new one.
3. In `Cubism Model`, click `Upload Model Zip`.
4. Choose a `.model3.zip` file.
5. Use the scale, zoom, and offset controls to frame the model.
6. Optionally set a default expression in `Cubism Model Expression`.
7. Optionally set `Cubism Emotion Map` so assistant emotion tags trigger expressions.

The uploaded zip is stored in IndexedDB and is exported with the contact when you create an `.openduck` file.

## Add A Bundled Character To The Repo

Bundled characters live in `characters/*.openduck`. Each `.openduck` file is a zip archive with this shape:

```text
contact.json
assets/<model-name>.model3.zip
```

`contact.json` stores the contact prompt, icon, voice metadata, and Live2D config. For a bundled Live2D character, the `cubismModel` block should point at the model zip inside the archive:

```json
{
  "cubismModel": {
    "source": "zip",
    "url": null,
    "zipId": null,
    "zipName": "natori_pro_t06.model3.zip",
    "zipPath": "assets/natori_pro_t06.model3.zip",
    "scale": 3.5,
    "emotionMap": {
      "angry": "Angry",
      "blush": "Blushing",
      "neutral": "Normal",
      "sad": "Sad",
      "smile": "Smile",
      "surprised": "Surprised"
    }
  }
}
```

After the `.openduck` file is placed in `characters/`, it is picked up by the app through the built-in character import glob in `src/lib/openduck/contacts.ts`.

## Expressions

Expression names come from `FileReferences.Expressions` in the model's `.model3.json`. Use the `Name` value, not the file path, when configuring OpenDuck.

For example, Jin Natori includes expression files such as:

```text
exp/Angry.exp3.json
exp/Blushing.exp3.json
exp/Normal.exp3.json
exp/Sad.exp3.json
exp/Smile.exp3.json
exp/Surprised.exp3.json
```

If the `.model3.json` declares these as `Name` values like `Angry`, `Blushing`, and `Smile`, those exact names are the values to put in `Cubism Model Expression` or in `emotionMap`.

`Cubism Model Expression` sets the default expression. `Cubism Emotion Map` maps assistant-visible emotion tags to Live2D expression names:

```json
{
  "smile": "Smile",
  "sad": "Sad",
  "surprised": "Surprised"
}
```

When an emotion map exists, OpenDuck tells the assistant it may include tags such as `[smile]` or `[sad]`. Before speech playback, OpenDuck strips those tags from the spoken text and switches the Live2D expression to the mapped expression.

## Packaging Checklist

- The uploaded file must be a zip archive.
- The zip must include exactly one intended `.model3.json` model manifest.
- All paths referenced by `.model3.json` must exist inside the zip.
- The `.moc3` must be compatible with the bundled Cubism Core runtime.
- Expression names in OpenDuck must match `.model3.json` expression `Name` values exactly.
- If the model came from a third party, confirm its license allows your intended use.
