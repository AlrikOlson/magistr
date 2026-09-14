"""Render flat 2D README diagrams and icons with Blender 5.2 LTS.

Run from the repository root:
    blender --background --python docs/assets/render_readme.py

The scene uses an orthographic camera, flat emission materials, and thin XY
geometry. It has no external asset dependencies.
"""

import math
from pathlib import Path

import bpy
from mathutils import Vector

OUTPUT = Path(__file__).resolve().parent
WIDTH = 1600
HEIGHT = 900

# Start from a clean scene.
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)


def material(name, color):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*color, 1.0)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.clear()
    emission = nodes.new("ShaderNodeEmission")
    emission.inputs["Color"].default_value = (*color, 1.0)
    emission.inputs["Strength"].default_value = 1.0
    output = nodes.new("ShaderNodeOutputMaterial")
    mat.node_tree.links.new(emission.outputs["Emission"], output.inputs["Surface"])
    return mat


NAVY = material("ink", (0.015, 0.025, 0.06))
PANEL = material("panel", (0.055, 0.085, 0.16))
PANEL_HI = material("panel highlight", (0.09, 0.14, 0.24))
WHITE = material("warm white", (0.88, 0.92, 0.96))
MUTED = material("muted text", (0.40, 0.52, 0.66))
CYAN = material("roadmap cyan", (0.10, 0.72, 0.90))
ORANGE = material("workflow orange", (1.0, 0.40, 0.12))
VIOLET = material("verification violet", (0.58, 0.34, 0.92))
GREEN = material("verified green", (0.18, 0.86, 0.58))
DARK = material("dark ink", (0.02, 0.04, 0.09))


def mark(obj, asset):
    obj["asset"] = asset
    return obj


def cube(name, x, y, w, h, mat, asset="hero", z=0.0, bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(location=(x, y, z))
    obj = bpy.context.object
    obj.name = name
    obj.scale = (w / 2, h / 2, 0.035)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        modifier = obj.modifiers.new("soft corners", "BEVEL")
        modifier.width = bevel
        modifier.segments = 5
    obj.data.materials.append(mat)
    return mark(obj, asset)


def bar(name, start, end, thickness, mat, asset="hero", z=0.08):
    start, end = Vector(start), Vector(end)
    delta = end - start
    length = delta.length
    midpoint = (start + end) / 2
    obj = cube(name, midpoint.x, midpoint.y, length, thickness, mat, asset, z)
    obj.rotation_euler[2] = math.atan2(delta.y, delta.x)
    return obj


def circle(name, x, y, radius, mat, asset="hero", z=0.12):
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=48, radius=radius, depth=0.07, location=(x, y, z)
    )
    obj = bpy.context.object
    obj.name = name
    obj.data.materials.append(mat)
    return mark(obj, asset)


def triangle(name, points, mat, asset="hero", z=0.12):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([(x, y, z) for x, y in points], [], [(0, 1, 2)])
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    obj.data.materials.append(mat)
    return mark(obj, asset)


def arrow(name, start, end, mat=CYAN, asset="hero"):
    bar(name + " shaft", start, end, 0.075, mat, asset, z=0.15)
    start, end = Vector(start), Vector(end)
    angle = math.atan2((end - start).y, (end - start).x)
    tip = end
    left = tip + Vector((math.cos(angle + 2.55) * 0.26, math.sin(angle + 2.55) * 0.26))
    right = tip + Vector((math.cos(angle - 2.55) * 0.26, math.sin(angle - 2.55) * 0.26))
    triangle(name + " head", [tip, left, right], mat, asset, z=0.17)


def text(name, body, x, y, size, mat=WHITE, asset="hero", align="CENTER"):
    bpy.ops.object.text_add(location=(x, y, 0.19))
    obj = bpy.context.object
    obj.name = name
    obj.data.body = body
    obj.data.align_x = align
    obj.data.align_y = "CENTER"
    obj.data.size = size
    obj.data.space_character = 1.05
    obj.data.extrude = 0
    obj.data.bevel_depth = 0
    obj.data.materials.append(mat)
    return mark(obj, asset)


def line_pair(name, x, y, widths, mat, asset):
    for i, width in enumerate(widths):
        bar(name + str(i), (x, y - i * 0.22), (x + width, y - i * 0.22), 0.075, mat, asset, z=0.16)


# Visual helpers keep the diagram language flat, literal, and self-explanatory.
GOLD = material("review gold", (0.96, 0.70, 0.18))


def card(asset, key, x, y, width, height, title, caption, accent):
    cube(key + " card", x, y, width, height, PANEL, asset, z=0.02, bevel=0.12)
    cube(key + " accent", x - width / 2 + 0.12, y, 0.10, height - 0.36, accent, asset, z=0.10, bevel=0.03)
    text(key + " title", title, x - width / 2 + 0.38, y + 0.34, 0.22, WHITE, asset, "LEFT")
    text(key + " caption", caption, x - width / 2 + 0.38, y - 0.34, 0.16, MUTED, asset, "LEFT")


def ai_symbol(asset, key, x, y, accent=ORANGE):
    circle(key + " core", x, y, 0.20, accent, asset, z=0.24)
    for i, (dx, dy) in enumerate([(-0.34, 0.18), (0.35, 0.20), (-0.34, -0.20), (0.35, -0.22)]):
        bar(key + " spoke " + str(i), (x, y), (x + dx * 0.72, y + dy * 0.72), 0.045, accent, asset, z=0.22)
        circle(key + " node " + str(i), x + dx, y + dy, 0.08, WHITE, asset, z=0.25)


def review_symbol(asset, key, x, y):
    circle(key + " ring", x, y, 0.25, GOLD, asset, z=0.22)
    bar(key + " check short", (x - 0.12, y - 0.01), (x - 0.02, y - 0.11), 0.07, GOLD, asset, z=0.27)
    bar(key + " check long", (x - 0.02, y - 0.11), (x + 0.16, y + 0.13), 0.07, GOLD, asset, z=0.27)


def roadmap_symbol(asset, key, x, y):
    route = [(x - 0.55, y - 0.18), (x - 0.20, y + 0.20), (x + 0.15, y - 0.06), (x + 0.55, y + 0.28)]
    for i, (start, end) in enumerate(zip(route, route[1:])):
        bar(key + " route " + str(i), start, end, 0.09, CYAN, asset, z=0.22)
    for i, (px, py) in enumerate(route):
        circle(key + " milestone " + str(i), px, py, 0.12, ORANGE if i == 3 else WHITE, asset, z=0.25)


def checklist_symbol(asset, key, x, y):
    for i, width in enumerate([0.66, 0.48, 0.58]):
        py = y + 0.24 - i * 0.22
        cube(key + " box " + str(i), x - 0.37, py, 0.14, 0.14, PANEL_HI, asset, z=0.21, bevel=0.02)
        if i == 0:
            bar(key + " tick short", (x - 0.39, py), (x - 0.34, py - 0.05), 0.035, ORANGE, asset, z=0.26)
            bar(key + " tick long", (x - 0.34, py - 0.05), (x - 0.25, py + 0.08), 0.035, ORANGE, asset, z=0.26)
        bar(key + " line " + str(i), (x - 0.12, py), (x - 0.12 + width, py), 0.06, WHITE if i == 0 else MUTED, asset, z=0.23)


def terminal_symbol(asset, key, x, y):
    text(key + " prompt", ">_", x - 0.55, y + 0.15, 0.30, VIOLET, asset, "LEFT")
    line_pair(key + " output ", x - 0.54, y - 0.08, [0.42, 0.62, 0.32], WHITE, asset)


def evidence_symbol(asset, key, x, y):
    cube(key + " page", x, y, 0.75, 0.95, PANEL_HI, asset, z=0.18, bevel=0.04)
    bar(key + " line 1", (x - 0.25, y + 0.20), (x + 0.24, y + 0.20), 0.045, MUTED, asset, z=0.24)
    bar(key + " line 2", (x - 0.25, y + 0.02), (x + 0.13, y + 0.02), 0.045, MUTED, asset, z=0.24)
    bar(key + " check short", (x - 0.24, y - 0.23), (x - 0.08, y - 0.37), 0.075, GREEN, asset, z=0.25)
    bar(key + " check long", (x - 0.08, y - 0.37), (x + 0.27, y + 0.08), 0.075, GREEN, asset, z=0.25)


# Overview: the user steers the roadmap in conversation, with review before execution.
cube("hero background", 0, 0, 16.9, 9.5, NAVY, "hero", z=-0.2)
text("hero eyebrow", "MAGISTR  /  ROADMAP + WORKFLOW", 0, 3.75, 0.30, MUTED)
text("hero title", "Talk to the agent, steer the roadmap", 0, 3.20, 0.50, WHITE)
text("hero subhead", "Talk through the direction, review the draft, then ship the next item", 0, 2.68, 0.24, MUTED)

hero_cards = [
    (-4.0, 0.75, "ROADMAP", "goals + dependencies", CYAN),
    (4.0, 0.75, "READY ITEM", "one focused change", ORANGE),
    (4.0, -2.20, "WORK", "agent with context", VIOLET),
    (-4.0, -2.20, "EVIDENCE", "checks + recorded reason", GREEN),
]
for index, (x, y, title, caption, accent) in enumerate(hero_cards):
    card("hero", "hero card " + str(index), x, y, 4.4, 2.0, title, caption, accent)

roadmap_symbol("hero", "hero roadmap", -2.85, 0.55)
checklist_symbol("hero", "hero ready", 5.05, 0.44)
terminal_symbol("hero", "hero work", 5.00, -2.00)
evidence_symbol("hero", "hero evidence", -2.60, -2.15)

arrow("hero top arrow", (-1.65, 0.75), (1.65, 0.75), ORANGE, "hero")
arrow("hero right arrow", (4.0, -0.33), (4.0, -1.20), VIOLET, "hero")
arrow("hero bottom arrow", (1.65, -2.20), (-1.65, -2.20), GREEN, "hero")
arrow("hero left arrow", (-4.0, -1.20), (-4.0, -0.33), CYAN, "hero")
text("hero proposal label", "YOU SET DIRECTION IN CHAT", 0, 1.35, 0.18, ORANGE, "hero")
cube("hero center panel", 0, -0.70, 4.9, 1.42, PANEL_HI, "hero", z=0.01, bevel=0.12)
ai_symbol("hero", "hero ai", -1.72, -0.70)
review_symbol("hero", "hero review", 1.72, -0.70)
text("hero center label", "AGENT DRAFTS THE PLAN", 0, -0.35, 0.24, WHITE, "hero")
text("hero center hint", "you choose what to apply", 0, -0.86, 0.18, MUTED, "hero")

# Transparent section icons use the same symbols at a compact scale.
cube("roadmap icon page", 0, 0, 3.45, 2.60, PANEL, "roadmap", z=0.02, bevel=0.16)
roadmap_symbol("roadmap", "roadmap icon route", 0, 0.15)
text("roadmap icon caption", "ROADMAP", 0, -1.35, 0.24, CYAN, "roadmap")

cube("workflow icon page", 0, 0, 3.45, 2.60, PANEL, "workflow", z=0.02, bevel=0.16)
checklist_symbol("workflow", "workflow icon checklist", 0, 0.15)
ai_symbol("workflow", "workflow icon ai", 0.75, -0.35, ORANGE)
text("workflow icon caption", "ROADMAP CHAT", 0, -1.35, 0.24, ORANGE, "workflow")

cube("verification icon page", 0, 0, 3.45, 2.60, PANEL, "verification", z=0.02, bevel=0.16)
evidence_symbol("verification", "verification icon evidence", 0, 0.15)
review_symbol("verification", "verification icon review", 0.78, -0.30)
text("verification icon caption", "VERIFY", 0, -1.35, 0.24, GREEN, "verification")

# Ownership boundary: you steer the roadmap through a local workflow you still control.
cube("system background", 0, 0, 13.6, 7.6, NAVY, "system", z=-0.2)
text("system eyebrow", "OWNERSHIP + CONTROL", 0, 3.1, 0.30, MUTED, "system")
text("system title", "Talk to the agent; keep control of the plan", 0, 2.6, 0.43, WHITE, "system")
system_cards = [
    (-4.75, 0.72, "YOU", "intent + review", CYAN),
    (-1.60, 0.72, "AGENT", "drafts from your direction", ORANGE),
    (1.70, 0.72, "MAGISTR", "orders work", VIOLET),
    (4.85, 0.72, "RECORD", "reason + checks", GREEN),
]
for index, (x, y, title, caption, accent) in enumerate(system_cards):
    card("system", "system card " + str(index), x, y, 2.60, 1.55, title, caption, accent)
circle("system you head", -5.18, 0.77, 0.13, WHITE, "system", z=0.24)
bar("system you body", (-5.18, 0.58), (-5.18, 0.36), 0.09, CYAN, "system", z=0.22)
ai_symbol("system", "system agent ai", -1.00, 0.70)
circle("system magistr core", 1.70, 0.70, 0.23, VIOLET, "system", z=0.24)
bar("system magistr spoke 1", (1.70, 0.70), (1.40, 0.97), 0.05, WHITE, "system", z=0.22)
bar("system magistr spoke 2", (1.70, 0.70), (2.00, 0.97), 0.05, WHITE, "system", z=0.22)
evidence_symbol("system", "system record evidence", 5.17, 0.70)
arrow("system intent arrow", (-3.42, 0.72), (-2.93, 0.72), CYAN, "system")
arrow("system proposal arrow", (-0.30, 0.72), (0.42, 0.72), ORANGE, "system")
arrow("system record arrow", (3.00, 0.72), (3.52, 0.72), VIOLET, "system")
text("system intent label", "set direction", -3.17, -0.16, 0.16, CYAN, "system")
text("system proposal label", "talk, review, apply", 0.06, -0.16, 0.16, ORANGE, "system")
text("system record label", "record outcome", 3.27, -0.16, 0.16, GREEN, "system")

cube("system checkout panel", -3.35, -1.48, 4.55, 1.22, PANEL, "system", z=0.02, bevel=0.10)
text("system checkout title", "YOUR PROJECT", -5.25, -1.25, 0.22, WHITE, "system", "LEFT")
text("system checkout caption", "source + commands", -5.25, -1.70, 0.18, MUTED, "system", "LEFT")
cube("system workflow panel", 3.25, -1.48, 4.55, 1.22, PANEL, "system", z=0.02, bevel=0.10)
text("system workflow title", "LOCAL WORKFLOW", 1.00, -1.25, 0.22, WHITE, "system", "LEFT")
text("system workflow caption", "roadmap + evidence", 1.00, -1.70, 0.18, MUTED, "system", "LEFT")
arrow("system return arrow", (3.25, -2.12), (-3.35, -2.12), GREEN, "system")
text("system return label", "updated roadmap is ready for the next decision", 0, -2.52, 0.19, GREEN, "system")
cube("system boundary note", 0, -3.03, 10.8, 0.72, PANEL_HI, "system", z=0.02, bevel=0.10)
text("system boundary note text", "Local by design · provider credentials and OS permissions stay with your runtime.", 0, -3.03, 0.19, WHITE, "system")

# Evidence chain: the record shows how a user-directed roadmap conversation becomes shipped work.
cube("evidence background", 0, 0, 15.0, 6.6, NAVY, "evidence", z=-0.2)
text("evidence eyebrow", "ROADMAP CHANGE → PROOF", 0, 2.55, 0.30, MUTED, "evidence")
text("evidence title", "Your roadmap conversation keeps the why", 0, 2.05, 0.42, WHITE, "evidence")
evidence_cards = [
    (-6.10, 0.20, "ROADMAP", "goals + deps", CYAN),
    (-3.66, 0.20, "AGENT DRAFT", "from your prompt", ORANGE),
    (-1.22, 0.20, "REVIEW", "you approve", GOLD),
    (1.22, 0.20, "READY TASK", "acceptance", VIOLET),
    (3.66, 0.20, "CHECKS", "gates pass", GREEN),
    (6.10, 0.20, "RECORD", "reason + result", WHITE),
]
for index, (x, y, title, caption, accent) in enumerate(evidence_cards):
    card("evidence", "evidence card " + str(index), x, y, 2.0, 1.55, title, caption, accent)
roadmap_symbol("evidence", "evidence roadmap", -5.55, 0.02)
ai_symbol("evidence", "evidence ai", -3.12, 0.00, ORANGE)
review_symbol("evidence", "evidence review", -0.70, 0.00)
checklist_symbol("evidence", "evidence ready", 1.82, 0.00)
review_symbol("evidence", "evidence checks", 4.22, 0.00)
evidence_symbol("evidence", "evidence record", 6.55, 0.00)
for i, (start_point, end_point, mat) in enumerate([
    ((-5.10, 0.20), (-4.66, 0.20), CYAN),
    ((-2.66, 0.20), (-2.22, 0.20), ORANGE),
    ((-0.22, 0.20), (0.22, 0.20), GOLD),
    ((2.22, 0.20), (2.66, 0.20), VIOLET),
    ((4.66, 0.20), (5.10, 0.20), GREEN),
]):
    arrow("evidence connector " + str(i), start_point, end_point, mat, "evidence")
text("evidence footer", "your prompt → draft → review → ready work → checks → recorded reason", 0, -1.35, 0.25, WHITE, "evidence")
cube("evidence loop panel", 0, -2.20, 9.5, 0.72, PANEL_HI, "evidence", z=0.02, bevel=0.10)
text("evidence loop text", "The next session starts with the updated roadmap and its evidence.", 0, -2.20, 0.20, GREEN, "evidence")

# Orthographic render settings. No lights are required for emission materials.
scene = bpy.context.scene
scene.render.engine = "BLENDER_EEVEE"
scene.render.resolution_x = WIDTH
scene.render.resolution_y = HEIGHT
scene.render.resolution_percentage = 100
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.image_settings.color_depth = "8"
scene.render.film_transparent = False
scene.render.filepath = str(OUTPUT / "magistr-roadmap-workflow.png")
scene.view_settings.look = "AgX - Medium High Contrast"

bpy.ops.object.camera_add(location=(0, 0, 10))
camera = bpy.context.object
camera.name = "orthographic camera"
camera.data.type = "ORTHO"
camera.data.ortho_scale = 16.9
camera.rotation_euler = (0, 0, 0)
scene.camera = camera

# Hero overview.
for obj in scene.objects:
    obj.hide_render = obj.get("asset") not in {"hero", None}
scene.render.film_transparent = False
scene.render.resolution_x = WIDTH
scene.render.resolution_y = HEIGHT
scene.render.filepath = str(OUTPUT / "magistr-roadmap-workflow.png")
bpy.ops.render.render(write_still=True)

# Transparent icons use the same flat source geometry.
icons = [("roadmap", 5.2), ("workflow", 5.0), ("verification", 5.0)]
scene.render.film_transparent = True
scene.render.resolution_x = 384
scene.render.resolution_y = 384
for asset, scale in icons:
    for obj in scene.objects:
        obj.hide_render = obj.get("asset") != asset
    camera.data.ortho_scale = scale
    scene.render.filepath = str(OUTPUT / (asset + ".png"))
    bpy.ops.render.render(write_still=True)

# Full-width diagrams use the same dark panel treatment and literal labels.
diagrams = [("system", 16.0), ("evidence", 16.0)]
scene.render.film_transparent = False
scene.render.resolution_x = 1200
scene.render.resolution_y = 700
for asset, scale in diagrams:
    for obj in scene.objects:
        obj.hide_render = obj.get("asset") != asset
    camera.data.ortho_scale = scale
    filename = {"system": "magistr-system-map.png", "evidence": "magistr-evidence-chain.png"}[asset]
    scene.render.filepath = str(OUTPUT / filename)
    bpy.ops.render.render(write_still=True)

