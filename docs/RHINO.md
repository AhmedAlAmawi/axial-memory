# Pictures of every step, in Rhino

`rhino/axial-capture.py` runs inside Rhino 7 or 8. It goes through a
Grasshopper definition in the order the data flows, shows only one
component's preview at a time, captures the Rhino view, and moves on, so the
pictures show how the geometry develops from the first inputs to the result.

Download: https://github.com/AhmedAlAmawi/axial-memory/releases/latest/download/axial-capture.py

## Using it

1. Open Rhino, then the definition in Grasshopper (and the Rhino file it uses, if any). Wait until it has computed.
2. Set up the view the pictures should use: camera, display mode, background. Click into that view once.
3. Type `RunPythonScript`, press Enter, and choose `axial-capture.py`.
4. It says how many pictures it will take and where they go. Click **Yes**. Don't touch Rhino until it finishes; **Esc** stops it.
5. A page opens showing the whole sequence. The folder is `Documents\Axial Memory captures\<definition> <date>`.

## What's in the folder

- `000 everything.png`: the definition as it was shown before the capture.
- `012 Pipe - Stalk.png`: step 12, only that component's geometry. Numbers follow the steps, so steps that make no geometry leave gaps.
- `index.html`: every step in order, pictures and data steps (sliders, numbers, lists) with their values.
- `manifest.json`: per step, the component, its group, outputs, warnings and errors, plus the camera, picture size, Rhino and Grasshopper versions and the definition's SHA-256.
- `log.txt`: what happened, for sending back if something goes wrong.

## What it changes

Nothing that lasts. It hides previews while it works, then puts back every
preview, the preview mode and filter, the selection and the "modified" flag.
It never saves the definition. If Grasshopper still asks to save when closing,
saying No loses nothing.

## Testing

`python3 rhino/test_axial_capture.py` tests the ordering, the one-component-
at-a-time loop and the output files without Rhino. Every Rhino and
Grasshopper call was checked against the Rhino 7 SDK (`Grasshopper` and
`RhinoCommon` 7.38 on NuGet). The capture itself has only run in Rhino on the
researcher's laptop.
