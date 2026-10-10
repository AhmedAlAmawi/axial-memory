#! python2
# -*- coding: utf-8 -*-
"""Axial Memory: a picture of every step of a Grasshopper definition.

Open the definition in Grasshopper, set up the Rhino view the pictures should
use, then run this file with RunPythonScript. Going through the components in
the order the data flows, it hides every preview except one component's and
captures the view (as ViewCaptureToFile does), then moves to the next. Steps
that make no geometry (numbers, lists) are written down with their values.

Nothing in the definition is changed or saved: every preview, the selection
and the modified flag are put back at the end, even if it stops early.
Written for Rhino 7 (IronPython 2.7); also runs in Rhino 8.
"""
from __future__ import division, print_function

import datetime
import hashlib
import heapq
import io
import json
import os
import re
import traceback

VERSION = "1.0"
TITLE = "Axial Memory"
# Pictures are this wide; the height follows the shape of the view.
PICTURE_WIDTH = 2400


def safe_name(text, limit=70):
    """Text usable as a file name on Windows and macOS."""
    cleaned = re.sub(r'[\\/:*?"<>|\x00-\x1f]', "_", text or "")
    cleaned = re.sub(r"\s+", " ", cleaned).strip(" .")
    return (cleaned[:limit].rstrip(" .") or "step")


def dataflow_order(ids, upstream, position):
    """Every id after everything that feeds it; otherwise left to right, then
    top to bottom on the canvas. Loops (rare) keep canvas order."""
    known = set(ids)
    waiting = dict((i, 0) for i in ids)
    feeds = dict((i, []) for i in ids)
    for i in ids:
        for u in set(upstream.get(i, ())):
            if u in known and u != i:
                waiting[i] += 1
                feeds[u].append(i)
    counter = [0]

    def entry(i):
        counter[0] += 1
        x, y = position.get(i, (0.0, 0.0))
        return (x, y, counter[0], i)

    heap = [entry(i) for i in ids if waiting[i] == 0]
    heapq.heapify(heap)
    order = []
    while heap:
        i = heapq.heappop(heap)[3]
        order.append(i)
        for j in feeds[i]:
            waiting[j] -= 1
            if waiting[j] == 0:
                heapq.heappush(heap, entry(j))
    placed = set(order)
    rest = sorted((i for i in ids if i not in placed), key=lambda i: position.get(i, (0.0, 0.0)))
    return order + rest


def run_sequence(stages, everything, hide, show, snap, should_stop):
    """Shows each stage alone and calls snap(stage) for it.

    `everything` is every object with a preview; all are hidden first. Returns
    (results, stopped) where results is a list of (stage, value or None,
    error text or None). Putting previews back is the caller's job.
    """
    for obj in everything:
        hide(obj)
    results = []
    for stage in stages:
        if should_stop():
            return results, True
        show(stage)
        try:
            results.append((stage, snap(stage), None))
        except Exception as e:
            results.append((stage, None, "%s: %s" % (type(e).__name__, e)))
        finally:
            hide(stage)
    return results, False


def _esc(text):
    return (u"%s" % (text,)).replace(u"&", u"&amp;").replace(u"<", u"&lt;").replace(u">", u"&gt;").replace(u'"', u"&quot;")


def contact_sheet(manifest):
    """A page showing the whole sequence, picture by picture."""
    m = manifest
    rows = []
    for step in m["steps"]:
        label = step["name"] + (u" \u201c%s\u201d" % step["nickname"] if step.get("nickname") and step["nickname"] != step["name"] else u"")
        where = u" \u00b7 %s" % step["group"] if step.get("group") else u""
        if step.get("picture"):
            rows.append(
                u'<figure><img src="%s" loading="lazy"><figcaption><b>%d.</b> %s<span>%s</span>%s</figcaption></figure>'
                % (_esc(step["picture"]), step["step"], _esc(label), _esc(where),
                   u"<em>%s</em>" % _esc(step["error"]) if step.get("error") else u"")
            )
        else:
            outputs = u"; ".join(
                u"%s %s%s" % (o["count"], o["type"], (u": " + u", ".join(o["values"])) if o.get("values") else u"")
                for o in step.get("outputs", [])
            )
            rows.append(u'<p class="data"><b>%d.</b> %s%s <span>%s</span></p>' % (step["step"], _esc(label), _esc(where), _esc(outputs)))
    style = (
        u"body{font:15px/1.5 -apple-system,'Segoe UI',sans-serif;margin:32px;background:#f5f2ea;color:#1d1c1a}"
        u"h1{font:500 30px Georgia,serif;margin:0 0 4px}.meta{color:#6f6a60;margin-bottom:24px}"
        u".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(340px,1fr));gap:18px}"
        u"figure{margin:0;background:#fffdf8;border:1px solid #e3ddd0;border-radius:12px;overflow:hidden}"
        u"figure img{display:block;width:100%;background:#fff}figcaption{padding:10px 14px}"
        u"figcaption span,.data span{color:#6f6a60}em{display:block;color:#a4472d;font-style:normal}"
        u".data{grid-column:1/-1;margin:0;padding:6px 14px;color:#1d1c1a;border-left:3px solid #e3ddd0}"
    )
    head = u"%d pictures, %d steps in all%s. Captured %s from %s." % (
        m["pictures"], len(m["steps"]), u" (stopped early)" if m.get("stopped") else u"", _esc(m["captured_at"]), _esc(m["definition"]["path"] or m["definition"]["name"])
    )
    return (
        u'<!doctype html><meta charset="utf-8"><title>%s</title><style>%s</style><h1>%s</h1><p class="meta">%s</p>'
        u'<p><a href="000 everything.png">Everything together</a></p><div class="grid">%s</div>'
        % (_esc(m["definition"]["name"]), style, _esc(m["definition"]["name"]), head, u"".join(rows))
    )


def file_sha256(path):
    if not path or not os.path.isfile(path):
        return None
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def write_text(path, text):
    with io.open(path, "w", encoding="utf-8") as f:
        f.write(u"%s" % text)


class Log(object):
    def __init__(self):
        self.lines = []

    def __call__(self, *parts):
        line = u" ".join(u"%s" % p for p in parts)
        self.lines.append(line)
        print(line)

    def save(self, folder):
        write_text(os.path.join(folder, "log.txt"), u"\n".join(self.lines) + u"\n")


def run():
    import clr
    import Rhino
    import rhinoscriptsyntax as rs
    import scriptcontext as sc
    import System

    plugin = Rhino.RhinoApp.GetPlugInObject("Grasshopper")
    if plugin is None or not plugin.IsEditorLoaded():
        rs.MessageBox("Open the definition in Grasshopper first, then run this again.", 48, TITLE)
        return
    clr.AddReference("System.Drawing")
    clr.AddReference("Grasshopper")
    import System.Drawing
    import Grasshopper
    from Grasshopper.Kernel import (
        GH_Document,
        GH_PreviewFilter,
        GH_PreviewMode,
        GH_RuntimeMessageLevel,
        IGH_ActiveObject,
        IGH_Component,
        IGH_Param,
        IGH_PreviewObject,
    )
    from Grasshopper.Kernel.Special import GH_Group

    log = Log()
    log("Axial Memory capture", VERSION, "| Rhino", Rhino.RhinoApp.Version, "| Grasshopper", Grasshopper.Versioning.Version)

    canvas = Grasshopper.Instances.ActiveCanvas
    doc = canvas.Document if canvas is not None else None
    if doc is None:
        rs.MessageBox("Open the definition in Grasshopper first, then run this again.", 48, TITLE)
        return
    view = sc.doc.Views.ActiveView
    if view is None:
        rs.MessageBox("Click into the Rhino view the pictures should use, then run this again.", 48, TITLE)
        return

    if not GH_Document.EnableSolutions:
        if rs.MessageBox("The Grasshopper solver is turned off, so the geometry may be out of date.\n\nTurn it on and compute the definition first?", 4 | 32, TITLE) != 6:
            return
        GH_Document.EnableSolutions = True
    doc.NewSolution(False)
    Rhino.RhinoApp.Wait()

    objects = list(doc.Objects)
    previewable = [o for o in objects if isinstance(o, IGH_PreviewObject)]
    active = [o for o in objects if isinstance(o, IGH_ActiveObject) and (isinstance(o, IGH_Component) or isinstance(o, IGH_Param))]

    def key(o):
        return str(o.InstanceGuid)

    def outputs_of(o):
        if isinstance(o, IGH_Component):
            return list(o.Params.Output)
        if isinstance(o, IGH_Param):
            return [o]
        return []

    def inputs_of(o):
        if isinstance(o, IGH_Component):
            return list(o.Params.Input)
        if isinstance(o, IGH_Param):
            return [o]
        return []

    upstream = {}
    position = {}
    for o in active:
        sources = []
        for p in inputs_of(o):
            for s in p.Sources:
                top = s.Attributes.GetTopLevel.DocObject
                if top is not None:
                    sources.append(key(top))
        upstream[key(o)] = sources
        pivot = o.Attributes.Pivot
        position[key(o)] = (float(pivot.X), float(pivot.Y))
    by_key = dict((key(o), o) for o in active)
    ordered = [by_key[k] for k in dataflow_order([key(o) for o in active], upstream, position)]

    # The innermost group a component sits in names its stage.
    group_of = {}
    group_size = {}
    for g in objects:
        if isinstance(g, GH_Group):
            ids = [str(i) for i in g.ObjectIDs]
            for i in ids:
                if i not in group_size or len(ids) < group_size[i]:
                    group_of[i] = g.NickName
                    group_size[i] = len(ids)

    def makes_geometry(o):
        try:
            return (isinstance(o, IGH_PreviewObject) and not o.Locked and o.IsPreviewCapable and o.ClippingBox.IsValid)
        except Exception:
            return False

    stages = [o for o in ordered if makes_geometry(o)]
    if not stages:
        rs.MessageBox("Nothing in this definition has geometry to show right now.\n\nCheck it has computed (no red components), then run this again.", 48, TITLE)
        return

    name = os.path.splitext(os.path.basename(doc.FilePath))[0] if doc.FilePath else (doc.DisplayName or "Untitled").rstrip("*")
    root = os.path.join(System.Environment.GetFolderPath(System.Environment.SpecialFolder.MyDocuments), "Axial Memory captures")
    folder = os.path.join(root, safe_name("%s %s" % (name, datetime.datetime.now().strftime("%Y-%m-%d %H.%M"))))
    question = (
        "%d of the %d steps in \"%s\" make geometry. Each one gets a picture of the view \"%s\", shown on its own.\n\n"
        "The pictures go to:\n%s\n\n"
        "Set the camera and display mode the way you want them first. Don't touch Rhino while it works; press Esc to stop.\n\n"
        "Start now?" % (len(stages), len(ordered), name, view.ActiveViewport.Name, folder)
    )
    if rs.MessageBox(question, 4 | 32, TITLE) != 6:
        return
    if not os.path.isdir(folder):
        os.makedirs(folder)

    vp = view.ActiveViewport
    size = vp.Size
    width = PICTURE_WIDTH
    height = int(round(width * size.Height / float(max(1, size.Width))))

    def capture(path):
        vc = Rhino.Display.ViewCapture()
        vc.Width = width
        vc.Height = height
        vc.ScaleScreenItems = False
        vc.DrawAxes = False
        vc.DrawGrid = False
        vc.DrawGridAxes = False
        vc.TransparentBackground = False
        bitmap = vc.CaptureToBitmap(view)
        if bitmap is None:
            raise RuntimeError("Rhino didn't return a picture of the view")
        try:
            bitmap.Save(path, System.Drawing.Imaging.ImageFormat.Png)
        finally:
            bitmap.Dispose()

    def refresh():
        sc.doc.Views.Redraw()
        Rhino.RhinoApp.Wait()

    def set_hidden(o, hidden):
        if o.Hidden != hidden:
            o.Hidden = hidden
            o.ExpirePreview(False)

    def output_summary(o):
        out = []
        for p in outputs_of(o):
            values = []
            try:
                for goo in p.VolatileData.AllData(True):
                    if len(values) >= 3:
                        break
                    values.append(u"%s" % goo)
            except Exception:
                pass
            out.append({"name": p.NickName, "type": p.TypeName, "count": p.VolatileDataCount, "values": values})
        return out

    def messages(o, level):
        try:
            return [u"%s" % m for m in o.RuntimeMessages(level)]
        except Exception:
            return []

    original_hidden = dict((key(o), o.Hidden) for o in previewable)
    original_mode = doc.PreviewMode
    original_filter = doc.PreviewFilter
    was_modified = doc.IsModified
    selected = [o for o in objects if o.Attributes.Selected]
    pictures = {}
    stopped = False
    Rhino.UI.StatusBar.ShowProgressMeter(0, len(stages), "Axial Memory", True, True)
    try:
        if original_mode == GH_PreviewMode.Disabled:
            doc.PreviewMode = GH_PreviewMode.Shaded
        doc.PreviewFilter = getattr(GH_PreviewFilter, "None")
        doc.DeselectAll()
        refresh()
        capture(os.path.join(folder, "000 everything.png"))
        log("000 everything.png: the definition as it was shown")

        # Pictures are numbered by step, so "012 ….png" is always step 12.
        step_of = dict((key(o), n) for n, o in enumerate(ordered, 1))
        done = [0]

        def snap(o):
            refresh()
            label = o.Name if not o.NickName or o.NickName == o.Name else "%s - %s" % (o.Name, o.NickName)
            file_name = "%03d %s.png" % (step_of[key(o)], safe_name(label))
            capture(os.path.join(folder, file_name))
            done[0] += 1
            Rhino.UI.StatusBar.UpdateProgressMeter(done[0], True)
            log(file_name)
            return file_name

        results, stopped = run_sequence(
            stages,
            previewable,
            lambda o: set_hidden(o, True),
            lambda o: set_hidden(o, False),
            snap,
            lambda: sc.escape_test(False),
        )
        for stage, file_name, error in results:
            pictures[key(stage)] = (file_name, error)
            if error:
                log("could not capture", stage.Name, ":", error)
    except Exception:
        log("stopped by an error:\n" + traceback.format_exc())
        stopped = True
    finally:
        for o in previewable:
            set_hidden(o, original_hidden.get(key(o), False))
        doc.PreviewMode = original_mode
        doc.PreviewFilter = original_filter
        for o in selected:
            o.Attributes.Selected = True
        doc.IsModified = was_modified
        Grasshopper.Instances.RedrawCanvas()
        refresh()
        Rhino.UI.StatusBar.HideProgressMeter()

    steps = []
    for n, o in enumerate(ordered, 1):
        file_name, error = pictures.get(key(o), (None, None))
        box = o.ClippingBox if isinstance(o, IGH_PreviewObject) and makes_geometry(o) else None
        steps.append({
            "step": n,
            "picture": file_name,
            "error": error,
            "id": key(o),
            "name": o.Name,
            "nickname": o.NickName,
            "category": o.Category,
            "subcategory": o.SubCategory,
            "group": group_of.get(key(o)),
            "outputs": output_summary(o),
            "warnings": messages(o, GH_RuntimeMessageLevel.Warning),
            "errors": messages(o, GH_RuntimeMessageLevel.Error),
            "bounding_box": [[box.Min.X, box.Min.Y, box.Min.Z], [box.Max.X, box.Max.Y, box.Max.Z]] if box is not None else None,
        })
    camera = {
        "view": vp.Name,
        "display_mode": vp.DisplayMode.EnglishName if vp.DisplayMode is not None else None,
        "location": [vp.CameraLocation.X, vp.CameraLocation.Y, vp.CameraLocation.Z],
        "target": [vp.CameraTarget.X, vp.CameraTarget.Y, vp.CameraTarget.Z],
        "lens_mm": vp.Camera35mmLensLength,
        "width": width,
        "height": height,
    }
    manifest = {
        "tool": "Axial Memory capture %s" % VERSION,
        "rhino": u"%s" % Rhino.RhinoApp.Version,
        "grasshopper": u"%s" % Grasshopper.Versioning.Version,
        "captured_at": datetime.datetime.now().strftime("%Y-%m-%d %H:%M"),
        "definition": {"name": name, "path": doc.FilePath, "sha256": file_sha256(doc.FilePath)},
        "camera": camera,
        "stopped": stopped,
        "pictures": sum(1 for s in steps if s["picture"]),
        "steps": steps,
    }
    write_text(os.path.join(folder, "manifest.json"), json.dumps(manifest, indent=2, ensure_ascii=False))
    write_text(os.path.join(folder, "index.html"), contact_sheet(manifest))
    log.save(folder)
    try:
        System.Diagnostics.Process.Start(os.path.join(folder, "index.html"))
    except Exception:
        pass
    rs.MessageBox(
        "%s %d pictures.\n\nThey're in:\n%s\n\nindex.html shows the whole sequence."
        % ("Stopped early after" if stopped else "Done:", manifest["pictures"], folder),
        64,
        TITLE,
    )


if __name__ == "__main__":
    try:
        run()
    except Exception:
        print(traceback.format_exc())
        import rhinoscriptsyntax as rs

        rs.MessageBox("Something went wrong. Please send Ahmed the text in Rhino's command line.", 16, TITLE)
