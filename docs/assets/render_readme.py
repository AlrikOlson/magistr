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


# Overview: a direct four-stage diagram.
cube("hero background", 0, 0, 16.9, 9.5, NAVY, "hero", z=-0.2)
text("hero eyebrow", "MAGISTR  /  ROADMAP + WORKFLOW", 0, 3.75, 0.30, MUTED)
text("hero title", "From idea to a verified next step", 0, 3.20, 0.50, WHITE)
text("hero subhead", "A project loop your agent can resume", 0, 2.68, 0.24, MUTED)

cards = [
    (-4.0, 0.75, "ROADMAP", "priorities + dependencies", CYAN),
    (4.0, 0.75, "PLAN", "one ready item", ORANGE),
    (4.0, -2.20, "WORK", "agent with context", VIOLET),
    (-4.0, -2.20, "VERIFY", "checks + project record", GREEN),
]
for x, y, title, caption, accent in cards:
    cube(title.lower() + " card", x, y, 4.4, 2.0, PANEL, "hero", z=0.02, bevel=0.14)
    cube(title.lower() + " accent", x - 1.92, y, 0.12, 1.55, accent, "hero", z=0.10, bevel=0.04)
    text(title.lower() + " title", title, x - 1.45, y + 0.55, 0.30, WHITE, "hero", "LEFT")
    text(title.lower() + " caption", caption, x - 1.45, y - 0.60, 0.19, MUTED, "hero", "LEFT")

# Roadmap card: a literal route with milestones.
bar("hero route 1", (-5.05, 0.35), (-4.30, 0.85), 0.09, CYAN, "hero", 0.17)
bar("hero route 2", (-4.30, 0.85), (-3.55, 0.45), 0.09, CYAN, "hero", 0.17)
bar("hero route 3", (-3.55, 0.45), (-2.95, 1.00), 0.09, CYAN, "hero", 0.17)
for i, (x, y) in enumerate([(-5.05, 0.35), (-4.30, 0.85), (-3.55, 0.45), (-2.95, 1.00)]):
    circle("hero milestone " + str(i), x, y, 0.13, WHITE, "hero", 0.22)

# Plan card: three task rows and a selected checkbox.
for i, (y, width) in enumerate([(0.72, 0.98), (0.50, 0.78), (0.28, 0.90)]):
    cube("hero checkbox " + str(i), 2.58, y, 0.18, 0.18, PANEL_HI, "hero", z=0.17, bevel=0.03)
    if i < 2:
        bar("hero check " + str(i), (2.61, y), (2.67, y - 0.06), 0.035, ORANGE, "hero", 0.24)
        bar("hero check 2 " + str(i), (2.67, y - 0.06), (2.76, y + 0.08), 0.035, ORANGE, "hero", 0.24)
    bar("hero task line " + str(i), (2.92, y), (2.92 + width, y), 0.07, WHITE if i == 0 else MUTED, "hero", 0.17)

# Work card: terminal prompt and three output lines.
text("hero prompt", ">_", 2.62, -1.92, 0.45, VIOLET, "hero", "LEFT")
line_pair("hero output ", 2.65, -2.17, [0.72, 1.05, 0.52], WHITE, "hero")

# Verify card: a document outline and a large check.
cube("hero document", -2.72, -2.15, 1.05, 1.32, PANEL_HI, "hero", z=0.16, bevel=0.05)
bar("hero document line 1", (-3.10, -1.78), (-2.44, -1.78), 0.06, MUTED, "hero", 0.23)
bar("hero document line 2", (-3.10, -1.98), (-2.52, -1.98), 0.06, MUTED, "hero", 0.23)
bar("hero verify short", (-3.08, -2.42), (-2.83, -2.65), 0.10, GREEN, "hero", 0.25)
bar("hero verify long", (-2.83, -2.65), (-2.35, -2.02), 0.10, GREEN, "hero", 0.25)

# Arrows make the loop direction explicit.
arrow("top arrow", (-1.65, 0.75), (1.65, 0.75), ORANGE, "hero")
arrow("right arrow", (4.0, -0.33), (4.0, -1.20), VIOLET, "hero")
arrow("bottom arrow", (1.65, -2.20), (-1.65, -2.20), GREEN, "hero")
arrow("left arrow", (-4.0, -1.20), (-4.0, -0.33), CYAN, "hero")
cube("hero center panel", 0, -0.70, 3.0, 1.35, PANEL, "hero", z=0.01, bevel=0.12)
text("hero center label", "CHOOSE THE NEXT ITEM", 0, -0.48, 0.22, WHITE)
text("hero center hint", "the project keeps moving", 0, -0.91, 0.19, MUTED)

# Transparent section icon: roadmap milestones.
cube("roadmap icon page", 0, 0, 3.45, 2.60, PANEL, "roadmap", z=0.02, bevel=0.16)
cube("roadmap icon fold", 1.21, 0.78, 0.55, 0.42, PANEL_HI, "roadmap", z=0.10, bevel=0.05)
route = [(-1.18, -0.72), (-0.35, 0.18), (0.38, -0.28), (1.10, 0.62)]
for i, (start, end) in enumerate(zip(route, route[1:])):
    bar("roadmap icon route " + str(i), start, end, 0.12, CYAN, "roadmap", z=0.18)
for i, (x, y) in enumerate(route):
    circle("roadmap icon milestone " + str(i), x, y, 0.20, ORANGE if i == 3 else WHITE, "roadmap", z=0.24)
text("roadmap icon caption", "ROADMAP", 0, -1.70, 0.24, CYAN, "roadmap")

# Transparent section icon: task cards.
for i, y in enumerate([0.72, 0.05, -0.62]):
    cube("workflow icon card " + str(i), 0.12 * i, y, 3.45, 0.54, PANEL if i else PANEL_HI, "workflow", z=0.03 + i * 0.04, bevel=0.08)
    cube("workflow icon checkbox " + str(i), -1.20 + 0.12 * i, y, 0.25, 0.25, ORANGE if i == 0 else CYAN, "workflow", z=0.20, bevel=0.04)
    bar("workflow icon line " + str(i), (-0.78 + 0.12 * i, y), (0.95 + 0.12 * i, y), 0.09, WHITE if i == 0 else MUTED, "workflow", z=0.22)
text("workflow icon caption", "WORKFLOW", 0, -1.58, 0.24, ORANGE, "workflow")

# Transparent section icon: checked document.
cube("verification icon page", 0, 0.18, 2.45, 3.05, PANEL, "verification", z=0.02, bevel=0.13)
cube("verification icon fold", 0.76, 1.28, 0.42, 0.38, PANEL_HI, "verification", z=0.10, bevel=0.04)
line_pair("verification icon lines ", -0.72, 0.72, [0.82, 0.60, 0.74], MUTED, "verification")
bar("verification icon check short", (-0.72, -0.35), (-0.30, -0.70), 0.15, GREEN, "verification", z=0.25)
bar("verification icon check long", (-0.30, -0.70), (0.65, 0.40), 0.15, GREEN, "verification", z=0.25)
text("verification icon caption", "VERIFY", 0, -1.72, 0.24, GREEN, "verification")

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

