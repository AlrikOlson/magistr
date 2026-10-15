// A bounded, on-demand WebGL material for the catalog ring. HTML owns all meaning.
const vertex = `
attribute vec2 position;
varying vec2 point;
void main() { point = position; gl_Position = vec4(position, 0.0, 1.0); }
`;
const fragment = `
precision mediump float;
varying vec2 point;
uniform vec4 ends;
uniform vec3 colors[4];
uniform float empty;
uniform float segments;
vec3 model(vec3 p) {
  // Orthographic camera: the chart's angular proportions remain source-owned.
  float c = 0.82, s = 0.5724;
  return vec3(p.x, c * p.y - s * p.z, s * p.y + c * p.z);
}
float surface(vec3 p) {
  vec3 q = model(p);
  return length(vec2(length(q.xy) - 0.66, q.z)) - 0.145;
}
void main() {
  vec3 origin = vec3(point.x * 1.03, (point.y - 0.23) * 1.03, 2.4);
  vec3 direction = vec3(0.0, 0.0, -1.0);
  float travel = 0.0;
  vec3 hit = origin;
  bool found = false;
  for (int i = 0; i < 48; i++) {
    hit = origin + direction * travel;
    float distance = surface(hit);
    if (distance < 0.0018) { found = true; break; }
    travel += max(distance, 0.001);
    if (travel > 4.0) break;
  }
  if (!found) discard;
  vec3 q = model(hit);
  float angle = fract(atan(q.x, q.y) / 6.2831853 + 1.0);
  float gap = 1.0;
  if (segments > 1.0) {
    float slot = fract(angle * segments);
    gap = smoothstep(0.001, 0.004, min(slot, 1.0 - slot) / segments);
  }
  vec3 color = colors[0];
  if (angle >= ends.x) color = colors[1];
  if (angle >= ends.y) color = colors[2];
  if (angle >= ends.z) color = colors[3];
  if (empty > 0.5) color = colors[2];
  vec2 e = vec2(0.003, 0.0);
  vec3 normal = normalize(vec3(surface(hit + e.xyy) - surface(hit - e.xyy),
    surface(hit + e.yxy) - surface(hit - e.yxy), surface(hit + e.yyx) - surface(hit - e.yyx)));
  vec3 light = normalize(vec3(-0.5, 0.7, 1.5));
  float diffuse = max(dot(normal, light), 0.0);
  float specular = pow(max(dot(normal, normalize(light - direction)), 0.0), 45.0);
  float rim = pow(1.0 - max(dot(normal, -direction), 0.0), 3.0);
  vec3 material = color * (0.48 + 0.52 * diffuse) + vec3(specular * 0.3 + rim * 0.08);
  gl_FragColor = vec4(material, gap);
}
`;
function rgb(value) {
  const hex = value.trim();
  if (/^#[0-9a-f]{6}$/i.test(hex)) return [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
  const channels = hex.match(/[\d.]+/g);
  return channels?.length >= 3 ? channels.slice(0, 3).map(Number).map(v => v / 255) : [0.5, 0.5, 0.5];
}
export class ActivityMaterial {
  constructor(root) {
    this.root = root; this.document = root.ownerDocument; this.disposed = false; this.counts = null;
    this.canvas = this.document.createElement("canvas");
    this.canvas.className = "activity-material";
    this.canvas.setAttribute("aria-hidden", "true");
    root.prepend(this.canvas);
    this.scheme = matchMedia("(prefers-color-scheme: light)");
    this.contrast = matchMedia("(forced-colors: active)");
    this.refresh = () => this.draw();
    this.lost = event => {
      event.preventDefault(); this.program = null; this.buffer = null; this.signature = null;
      delete root.dataset.material;
    };
    this.restored = () => { this.initialize(); this.draw(); };
    this.canvas.addEventListener("webglcontextlost", this.lost);
    this.canvas.addEventListener("webglcontextrestored", this.restored);
    this.resize = new ResizeObserver(this.refresh); this.resize.observe(root);
    this.theme = new MutationObserver(this.refresh);
    this.theme.observe(this.document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    this.scheme.addEventListener("change", this.refresh); this.contrast.addEventListener("change", this.refresh);
    this.document.addEventListener("visibilitychange", this.refresh);
    this.onPageHide = event => { if (!event.persisted) this.dispose(); };
    this.document.defaultView.addEventListener("pagehide", this.onPageHide);
    this.initialize();
  }
  initialize() {
    if (this.disposed) return;
    const gl = this.gl ?? this.canvas.getContext("webgl", { alpha: true, antialias: true, depth: false, stencil: false, powerPreference: "low-power", preserveDrawingBuffer: false });
    if (!gl || gl.isContextLost()) return;
    this.gl = gl;
    const shaders = [];
    let program = null, buffer = null;
    try {
      for (const [type, source] of [[gl.VERTEX_SHADER, vertex], [gl.FRAGMENT_SHADER, fragment]]) {
        const shader = gl.createShader(type);
        if (!shader) throw Error("Shader unavailable");
        shaders.push(shader); gl.shaderSource(shader, source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw Error("Shader compilation failed");
      }
      program = gl.createProgram();
      if (!program) throw Error("Program unavailable");
      for (const shader of shaders) gl.attachShader(program, shader);
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw Error("Shader link failed");
      buffer = gl.createBuffer();
      if (!buffer) throw Error("Buffer unavailable");
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1,1,-1,-1,1,-1,1,1,-1,1,1]), gl.STATIC_DRAW);
      this.program = program; this.buffer = buffer; this.signature = null;
      this.locations = Object.fromEntries(["ends","colors[0]","empty","segments"].map(name => [name, gl.getUniformLocation(program,name)]));
      this.position = gl.getAttribLocation(program,"position");
    } catch {
      if (program) gl.deleteProgram(program);
      if (buffer) gl.deleteBuffer(buffer);
      this.program = null; this.buffer = null; delete this.root.dataset.material;
    } finally { for (const shader of shaders) gl.deleteShader(shader); }
  }
  update(counts) { this.counts = counts; this.draw(); }
  draw() {
    const gl = this.gl;
    if (this.disposed || !gl || !this.program || !this.counts || gl.isContextLost()) return;
    if (this.contrast.matches) { delete this.root.dataset.material; return; }
    if (this.document.hidden || !this.root.clientWidth || !this.root.clientHeight) return;
    const size = Math.min(384, Math.round(this.root.clientWidth * Math.min(devicePixelRatio || 1, 2)));
    if (!size) return;
    const style = getComputedStyle(this.root);
    const palette = ["success", "warning", "muted", "danger"].map(name => style.getPropertyValue("--" + name));
    const signature = JSON.stringify([size,this.counts,palette]);
    if (this.signature === signature && this.root.dataset.material === "rendered") return;
    this.signature = signature;
    if (this.canvas.width !== size) this.canvas.width = this.canvas.height = size;
    gl.viewport(0, 0, size, size); gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program); gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    const position = this.position;
    gl.enableVertexAttribArray(position); gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    const total = this.counts.reduce((sum, count) => sum + count, 0);
    let sum = 0;
    const ends = this.counts.map(count => { sum += count; return total ? sum / total : 1; });
    const colors = palette.flatMap(rgb);
    gl.uniform4fv(this.locations["ends"], ends);
    gl.uniform3fv(this.locations["colors[0]"], colors);
    gl.uniform1f(this.locations["empty"], total ? 0 : 1);
    gl.uniform1f(this.locations["segments"], total <= 64 ? total : 0);
    gl.drawArrays(gl.TRIANGLES, 0, 6); gl.flush();
    this.root.dataset.material = "rendered";
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true; this.resize.disconnect(); this.theme.disconnect();
    this.scheme.removeEventListener("change", this.refresh); this.contrast.removeEventListener("change", this.refresh);
    this.document.removeEventListener("visibilitychange", this.refresh);
    this.document.defaultView.removeEventListener("pagehide", this.onPageHide);
    this.canvas.removeEventListener("webglcontextlost", this.lost); this.canvas.removeEventListener("webglcontextrestored", this.restored);
    if (this.program) this.gl.deleteProgram(this.program);
    if (this.buffer) this.gl.deleteBuffer(this.buffer);
    this.gl?.getExtension("WEBGL_lose_context")?.loseContext();
    this.canvas.remove(); delete this.root.dataset.material;
  }
}
